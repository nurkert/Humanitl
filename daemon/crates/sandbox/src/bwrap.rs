//! Das `bwrap`-Backend (ADR-002, HUM-011, HUM-013).
//!
//! [`BwrapBackend`] findet `bwrap`, prüft Version und Nutzer-Namensräume,
//! baut aus Profil und Sitzung den [`LaunchPlan`] und startet ihn. Drei Dinge
//! sind hier anders als in einem gewöhnlichen `Command::spawn`, und jedes hat
//! einen Grund:
//!
//! - **Leere Umgebung.** `bwrap` bekommt keine einzige Variable des Hosts.
//!   `--clearenv` räumt nur die Umgebung des Befehls in der Sandbox auf;
//!   `bwrap` selbst bleibt als PID 1 des Namensraums stehen, und
//!   `/proc/1/environ` zeigte sonst Tokens, `DISPLAY` und
//!   `DBUS_SESSION_BUS_ADDRESS` des Nutzers (ESC-2-Befund, CONVENTIONS 4.11).
//! - **Geerbte Deskriptoren, keine Umnummerierung.** Die Masken und die drei
//!   Identitätsdateien kommen aus versiegelten memfds, einer je Datei
//!   (`bwrap` schließt jeden nach dem Lesen); der Bericht des Shims und der
//!   Status von `bwrap` über je eine Pipe. Die Deskriptoren werden
//!   unmittelbar vor dem Start von `FD_CLOEXEC` befreit und unter ihrer
//!   eigenen Nummer vererbt; ein `dup2` im Kind bräuchte `pre_exec`, also
//!   `unsafe`, und das ist in dieser Crate verboten. Ein prozessweiter Riegel
//!   hält das Fenster, in dem ein fremder `spawn` sie miterben könnte, so
//!   kurz wie möglich.
//! - **Ein Thread je Sandbox.** `--die-with-parent` ist `PR_SET_PDEATHSIG`,
//!   und das Signal kommt, wenn der *Thread* endet, der `bwrap` gestartet
//!   hat. Ein Thread aus einem Pool, der nach zehn Sekunden Leerlauf stirbt,
//!   nähme die Sandbox mit. Deshalb startet [`BwrapBackend::launch`] einen
//!   eigenen Thread, der `bwrap` erzeugt und auf es wartet, und nichts sonst.
//!
//! Ob `bwrap` seinen Befehl überhaupt gestartet hat, sagt am Ende die Zeile
//! `{"exit-code": …}` auf `--json-status-fd`; die Zeile `{"child-pid": …}`
//! kommt schon vor den Mounts und beweist nichts (siehe [`crate::handle`]).
//! [`BwrapBackend::launch`] wartet deshalb [`EARLY_EXIT_WINDOW`] lang auf ein
//! frühes Ende oder die erste Zeile des Shim-Berichts, und
//! [`SandboxHandle::wait`] fällt den Befund für alles, was später scheitert.

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::File;
use std::io::{BufRead, BufReader, PipeReader, Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, AsRawFd, OwnedFd, RawFd};
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::Duration;

use humanitl_config::{Env, Paths, WorkMode};
use humanitl_core::diagnostics::codes::{
    SANDBOX_001, SANDBOX_002, SANDBOX_003, SANDBOX_005, SANDBOX_006, SANDBOX_011, SANDBOX_012,
    SANDBOX_013, SANDBOX_014, SANDBOX_015, SANDBOX_016,
};
use humanitl_core::ids::SandboxId;
use humanitl_core::{Diagnostic, DiagnosticCode, FixAction, Severity};
use rustix::fs::{Access, MemfdFlags, SealFlags, access, fcntl_add_seals, memfd_create};
use rustix::io::{FdFlags, fcntl_setfd};

use crate::bridge_env::{
    CHECK_BRIDGE_LISTENING, CHECK_FAMILIES, CHECK_NO_INTERFACES, CHECK_SECCOMP_APPLIED,
    CHECK_SINGLE_SOCKET, parse_check_line,
};
use crate::bwrap_args::{IdentityFds, LaunchInputs, MaskFds};
use crate::handle::{OutputSink, ReportSnapshot, SandboxHandle, Shared};
use crate::launcher::{
    CheckResult, IsolationCheck, LaunchOnce, LaunchPlan, SandboxBackend, StdioMode,
};
use crate::profile::{MountPolicy, SandboxProfile, SessionContext, WORK_DST, normalize};

/// Die kleinste `bwrap`-Fassung, mit der der Launcher arbeitet.
///
/// `--disable-userns` gibt es seit 0.6, `--json-status-fd` und
/// `--ro-bind-data` länger; 0.8 ist, was Debian 12 und Ubuntu 24.04
/// ausliefern, und was das Profil als Untergrenze nennt.
pub const MIN_BWRAP_VERSION: Version = Version(0, 8, 0);

/// Wie lange [`BwrapBackend::launch`] nach dem Start auf ein frühes Ende von
/// `bwrap` wartet, bevor es die Sandbox als laufend meldet (HUM-011: 500 ms).
/// Die erste Zeile des Shim-Berichts beendet das Warten früher: sie kommt
/// erst, wenn der Befehl läuft.
pub const EARLY_EXIT_WINDOW: Duration = Duration::from_millis(500);

/// Wie lange [`SandboxBackend::isolation_check`] auf den Bericht des Shims wartet.
pub const REPORT_TIMEOUT: Duration = Duration::from_secs(5);

/// Der Befehl, der `bubblewrap` auf Debian und Ubuntu nachinstalliert.
pub const INSTALL_COMMAND: &str = "sudo apt install bubblewrap";

/// Der Befehl, der die AppArmor-Sperre für Nutzer-Namensräume aufhebt (Ubuntu ≥ 24.04).
pub const USERNS_SYSCTL_COMMAND: &str =
    "sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0";

/// Wo die Sperre erklärt ist.
pub const USERNS_DOCS_URL: &str =
    "https://ubuntu.com/blog/ubuntu-23-10-restricted-unprivileged-user-namespaces";

/// Die Deskriptoren der Adapter-Dateien und ihre Ziele in der Sandbox.
///
/// Der erste Teil hält die memfds am Leben, bis `bwrap` sie geerbt hat; der
/// zweite ist das, was die Argumentliste braucht.
type AgentFileFds = (Vec<OwnedFd>, Vec<(RawFd, PathBuf)>);

/// Der Name des memfd einer Adapter-Datei, wie ihn `/proc/<pid>/fd` zeigt.
const AGENT_FILE_MEMFD_NAME: &str = "humanitl-agent-file";

/// Der Name des memfd einer Maske, wie ihn `/proc/<pid>/fd` zeigt.
const MASK_MEMFD_NAME: &str = "humanitl-mask";

/// Wenn `PATH` fehlt: wo `bwrap` üblicherweise liegt.
const DEFAULT_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Hält das Fenster zwischen „CLOEXEC entfernt" und „gestartet" prozessweit
/// exklusiv, siehe Modulbeschreibung.
static SPAWN_LOCK: Mutex<()> = Mutex::new(());

/// Eine `bwrap`-Fassung als Zahlentripel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub u32, pub u32, pub u32);

impl Version {
    /// Liest `0.11.2`, `bubblewrap 0.11.2` oder `0.8`; was nicht lesbar ist,
    /// zählt als 0. Das genügt: verglichen werden schlichte Zahlenfolgen.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let mut parts = text
            .split(|c: char| !c.is_ascii_digit())
            .filter(|part| !part.is_empty())
            .map(|part| part.parse::<u32>().unwrap_or(0));
        Self(
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
            parts.next().unwrap_or(0),
        )
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Das `bwrap`-Backend.
#[derive(Debug, Clone)]
pub struct BwrapBackend {
    program: PathBuf,
    version: Version,
    paths: Paths,
    stdio: StdioMode,
    report_timeout: Duration,
    /// Wohin die gesammelte Ausgabe zusätzlich geht, während sie läuft.
    output_sink: Option<OutputSink>,
}

impl BwrapBackend {
    /// Sucht `bwrap` über `PATH` aus den übergebenen Pfaden, prüft Version und
    /// Nutzer-Namensräume.
    ///
    /// # Errors
    ///
    /// `SANDBOX_001` ohne `bwrap`, `SANDBOX_002` unter [`MIN_BWRAP_VERSION`],
    /// `SANDBOX_003`, wenn der Kernel keinen unprivilegierten
    /// Nutzer-Namensraum erlaubt.
    pub fn detect(paths: Paths) -> Result<Self, Diagnostic> {
        let program = Self::find_program(paths.env())?;
        Self::detect_program(paths, program)
    }

    /// Wie [`BwrapBackend::detect`], mit einem gegebenen Programm statt der Suche.
    ///
    /// # Errors
    ///
    /// Wie [`BwrapBackend::detect`], ohne den Fall „nicht gefunden" der Suche.
    pub fn detect_program(paths: Paths, program: PathBuf) -> Result<Self, Diagnostic> {
        let version = Self::query_version(&program)?;
        if version < MIN_BWRAP_VERSION {
            return Err(Diagnostic::builder(SANDBOX_002, Severity::Blocking)
                .why(format!(
                    "{} is bubblewrap {version}; the launcher needs at least {MIN_BWRAP_VERSION}",
                    program.display()
                ))
                .fix(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
                .build());
        }
        Self::probe_user_namespaces(&program)?;
        Ok(Self::unchecked(program, version, paths))
    }

    /// Ein Backend ohne jede Prüfung. Für Tests und für Aufrufer, die die
    /// Prüfungen selbst gemacht haben.
    #[must_use]
    pub fn unchecked(program: impl Into<PathBuf>, version: Version, paths: Paths) -> Self {
        Self {
            program: program.into(),
            version,
            paths,
            stdio: StdioMode::default(),
            report_timeout: REPORT_TIMEOUT,
            output_sink: None,
        }
    }

    /// Wohin Ein- und Ausgabe der Sandbox gehen; Standard [`StdioMode::Inherit`].
    #[must_use]
    pub const fn with_stdio(mut self, stdio: StdioMode) -> Self {
        self.stdio = stdio;
        self
    }

    /// Wohin jedes gelesene Stück Ausgabe zusätzlich geht, während die Sandbox
    /// läuft.
    ///
    /// Wirkt nur mit [`StdioMode::Capture`]; ohne Pipes gibt es nichts zu
    /// lesen. Der Sender wird vor dem Start gesetzt und nicht danach, damit
    /// kein Byte zwischen Start und Anmeldung verlorengeht.
    #[must_use]
    pub fn with_output_sink(mut self, sink: OutputSink) -> Self {
        self.output_sink = Some(sink);
        self
    }

    /// Wie lange [`SandboxBackend::isolation_check`] auf den Bericht wartet.
    #[must_use]
    pub const fn with_report_timeout(mut self, timeout: Duration) -> Self {
        self.report_timeout = timeout;
        self
    }

    /// Das Programm, das startet.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
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

    /// Der Modus für Ein- und Ausgabe.
    #[must_use]
    pub const fn stdio(&self) -> StdioMode {
        self.stdio
    }

    /// Sucht `bwrap` in `PATH` der übergebenen Umgebung, nie in der des
    /// Prozesses; ohne `PATH` in `/usr/local/bin:/usr/bin:/bin`.
    ///
    /// # Errors
    ///
    /// `SANDBOX_001`, wenn keine ausführbare Datei `bwrap` dort liegt.
    pub fn find_program(env: &Env) -> Result<PathBuf, Diagnostic> {
        let path = env.non_empty("PATH").unwrap_or(DEFAULT_PATH);
        for dir in path.split(':').filter(|dir| !dir.is_empty()) {
            let candidate = Path::new(dir).join("bwrap");
            if candidate.is_file() && access(&candidate, Access::EXEC_OK).is_ok() {
                return Ok(candidate);
            }
        }
        Err(Diagnostic::builder(SANDBOX_001, Severity::Blocking)
            .why(format!(
                "no executable bwrap in PATH={path}; bubblewrap is not installed"
            ))
            .fix(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
            .docs("https://github.com/containers/bubblewrap")
            .build())
    }

    /// Fragt `bwrap --version`.
    ///
    /// # Errors
    ///
    /// `SANDBOX_001`, wenn das Programm nicht läuft.
    pub fn query_version(program: &Path) -> Result<Version, Diagnostic> {
        let output = Self::scrubbed_command(program)
            .arg("--version")
            .stdin(Stdio::null())
            .output()
            .map_err(|err| {
                Diagnostic::builder(SANDBOX_001, Severity::Blocking)
                    .why(format!("cannot run {} --version: {err}", program.display()))
                    .fix(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
                    .build()
            })?;
        let text = String::from_utf8_lossy(&output.stdout);
        Ok(Version::parse(text.trim()))
    }

    /// Startet `bwrap --unshare-user … /bin/true` und liest, ob der Kernel
    /// unprivilegierte Nutzer-Namensräume erlaubt.
    ///
    /// Nur die bekannten Fehlerbilder von `bwrap` (`setting up uid map`,
    /// `Creating new namespace failed`, `No permissions to create a new
    /// namespace`) sind ein Befund; scheitert die Probe aus einem anderen
    /// Grund, entscheidet der echte Start, nicht die Probe.
    ///
    /// # Errors
    ///
    /// `SANDBOX_003` mit dem Befehl, der die Sperre aufhebt.
    pub fn probe_user_namespaces(program: &Path) -> Result<(), Diagnostic> {
        let output = Self::scrubbed_command(program)
            .args([
                "--unshare-user",
                "--unshare-pid",
                "--unshare-net",
                "--die-with-parent",
                "--ro-bind",
                "/usr",
                "/usr",
                "--ro-bind-try",
                "/bin",
                "/bin",
                "--ro-bind-try",
                "/lib",
                "/lib",
                "--ro-bind-try",
                "/lib64",
                "/lib64",
                "--ro-bind-try",
                "/etc/ld.so.cache",
                "/etc/ld.so.cache",
                "--",
                "/bin/true",
            ])
            .stdin(Stdio::null())
            .output();
        let Ok(output) = output else {
            // Ob es läuft, hat `query_version` schon beantwortet.
            return Ok(());
        };
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if is_userns_failure(&stderr) {
            return Err(userns_diagnostic(stderr.trim()));
        }
        Ok(())
    }

    /// Ein `Command` für `program` mit leerer Umgebung.
    ///
    /// Jeder Start von `bwrap`, auch `--version` und die Probe, geht hier
    /// durch: nichts vom Host, nie.
    #[must_use]
    pub fn scrubbed_command(program: &Path) -> Command {
        let mut command = Command::new(program);
        command.env_clear();
        command
    }

    fn check_work_dir(
        profile: &SandboxProfile,
        session: &SessionContext,
    ) -> Result<(), Diagnostic> {
        let work = session.work_src.as_path();
        let shown = work.display();
        if !work.is_absolute() {
            return Err(work_dir_diagnostic(format!(
                "sandbox.work_dir {shown} is not an absolute path"
            )));
        }
        let meta = std::fs::metadata(work).map_err(|err| {
            work_dir_diagnostic(format!("sandbox.work_dir {shown} does not exist: {err}"))
        })?;
        if !meta.is_dir() {
            return Err(work_dir_diagnostic(format!(
                "sandbox.work_dir {shown} is not a directory"
            )));
        }
        if profile.effective_work_mode(session.work_mode) == WorkMode::Rw
            && access(work, Access::WRITE_OK).is_err()
        {
            return Err(Diagnostic::builder(SANDBOX_005, Severity::Blocking)
                .why(format!(
                    "sandbox.work_dir {shown} is not writable, but sandbox.work_mode is rw"
                ))
                .fix(FixAction::RemountReadOnly(work.to_path_buf()))
                .build());
        }
        Ok(())
    }

    fn check_proxy_socket(&self, session: &SessionContext) -> Result<(), Diagnostic> {
        let socket = session.proxy_socket_src.as_path();
        let shown = socket.display();
        let meta = std::fs::symlink_metadata(socket).map_err(|err| {
            placeholder_diagnostic(format!(
                "proxy socket {shown} is missing: {err}; the daemon binds it before the sandbox starts"
            ))
        })?;
        if !meta.file_type().is_socket() {
            return Err(placeholder_diagnostic(format!(
                "proxy socket {shown} is not a Unix socket"
            )));
        }
        // Eine Tür für genau einen: der Daemon legt den Socket mit 0600 an
        // (`humanitl_config::FILE_MODE`); einer, den Gruppe oder Welt öffnen
        // dürfen, wird nicht eingehängt.
        let mode = meta.permissions().mode() & 0o777;
        if mode & 0o077 != 0 {
            return Err(Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                .why(format!(
                    "proxy socket {shown} has mode {mode:04o}; the one door into the sandbox must be 0600, owned by the daemon's user"
                ))
                .build());
        }
        // Nur die Datei aus dem Proxy-Verzeichnis, nie eine andere: der
        // gRPC-Socket daneben ist die Steuer-API (HUM-013).
        let dir = normalize(&self.paths.proxy_socket_dir());
        let written = normalize(socket);
        if !socket.is_absolute() || !written.starts_with(&dir) || written == dir {
            return Err(Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                .why(format!(
                    "proxy socket {shown} is outside {}; only a socket file from the proxy directory is mounted into the sandbox",
                    dir.display()
                ))
                .build());
        }
        Ok(())
    }

    fn check_regular_file(path: &Path, what: &str) -> Result<(), Diagnostic> {
        let shown = path.display();
        let meta = std::fs::metadata(path)
            .map_err(|err| placeholder_diagnostic(format!("{what} {shown} is missing: {err}")))?;
        if !meta.is_file() {
            return Err(placeholder_diagnostic(format!(
                "{what} {shown} is not a regular file"
            )));
        }
        Ok(())
    }

    fn check_shim(session: &SessionContext) -> Result<(), Diagnostic> {
        let shim = session.shim_src.as_path();
        Self::check_regular_file(shim, "shim binary")?;
        if access(shim, Access::EXEC_OK).is_err() {
            return Err(placeholder_diagnostic(format!(
                "shim binary {} is not executable",
                shim.display()
            )));
        }
        Ok(())
    }

    /// Die Pfade unter `/work` (Sandbox-Sicht), die es im Projekt gibt: ein
    /// Verzeichnis je `mounts.tmpfs` darunter, eine Datei je Maske.
    fn present_under_work(profile: &SandboxProfile, work_src: &Path) -> BTreeSet<PathBuf> {
        let work_dst = profile.mounts.work.dst.as_path();
        let mut present = BTreeSet::new();
        for path in &profile.mounts.tmpfs {
            if let Ok(rel) = path.strip_prefix(work_dst)
                && !rel.as_os_str().is_empty()
                && work_src.join(rel).is_dir()
            {
                present.insert(path.clone());
            }
        }
        for path in profile.effective_masked_files() {
            if let Ok(rel) = path.strip_prefix(work_dst)
                && !rel.as_os_str().is_empty()
                && work_src.join(rel).is_file()
            {
                present.insert(path);
            }
        }
        present
    }

    /// Ein memfd je Datei des Agent-Adapters, mit ihrem Ziel in der Sandbox.
    ///
    /// # Errors
    ///
    /// `SANDBOX_006`, wenn eine Datei unter `/work` liegen soll — Humanitl
    /// schreibt nicht in ein Repository, das ihm nicht gehört — oder wenn sie
    /// ein Ziel belegt, das die Sandbox selbst setzt (Proxy-Socket, CA, Shim,
    /// Identitätsdateien, `/proc`, `/sys`, `/dev`, `/run/humanitl`).
    /// `SANDBOX_011`, wenn ein memfd nicht anzulegen ist.
    fn agent_file_fds(session: &SessionContext) -> Result<AgentFileFds, Diagnostic> {
        let inside = crate::agent::files_inside_work(&session.files, Path::new(WORK_DST));
        if let Some(first) = inside.first() {
            return Err(Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                .why(format!(
                    "the agent adapter wants to write {} into the project directory {WORK_DST}; \
                     adapter files live outside it",
                    first.display()
                ))
                .build());
        }
        let reserved = crate::agent::files_on_reserved_targets(&session.files);
        if let Some(first) = reserved.first() {
            return Err(Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                .why(format!(
                    "the agent adapter wants to put a file at {}, a target the sandbox sets \
                     itself; its file would come later in the argument list and cover it",
                    first.display()
                ))
                .build());
        }
        let fds = session
            .files
            .iter()
            .map(|file| Self::sealed_memfd(AGENT_FILE_MEMFD_NAME, &file.content))
            .collect::<Result<Vec<OwnedFd>, Diagnostic>>()?;
        let targets = fds
            .iter()
            .zip(&session.files)
            .map(|(fd, file)| (fd.as_raw_fd(), file.dst.clone()))
            .collect();
        Ok((fds, targets))
    }

    /// Ein versiegeltes memfd mit diesem Inhalt, Leseposition am Anfang.
    ///
    /// Versiegelt gegen Wachsen, Schrumpfen und Schreiben: wer den Deskriptor
    /// erbt, kann den Inhalt nicht mehr ändern, und `bwrap` liest genau das,
    /// was der Launcher hineingelegt hat.
    fn sealed_memfd(name: &str, content: &[u8]) -> Result<OwnedFd, Diagnostic> {
        let fd = memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
            .map_err(|err| placeholder_diagnostic(format!("cannot create memfd {name}: {err}")))?;
        let mut file = File::from(fd);
        file.write_all(content)
            .and_then(|()| file.seek(SeekFrom::Start(0)))
            .map_err(|err| placeholder_diagnostic(format!("cannot fill memfd {name}: {err}")))?;
        let fd = OwnedFd::from(file);
        fcntl_add_seals(
            &fd,
            SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
        )
        .map_err(|err| placeholder_diagnostic(format!("cannot seal memfd {name}: {err}")))?;
        Ok(fd)
    }

    fn already_launched(plan: &LaunchPlan) -> Diagnostic {
        Diagnostic::builder(SANDBOX_012, Severity::Blocking)
            .why(format!(
                "the plan for session {} was already launched; a plan carries one report pipe and starts one sandbox",
                plan.session
            ))
            .build()
    }
}

impl SandboxBackend for BwrapBackend {
    fn name(&self) -> &'static str {
        "bwrap"
    }

    /// Prüft und plant.
    ///
    /// # Errors
    ///
    /// - `SANDBOX_006`: eine Quelle des Profils, das Projektverzeichnis oder
    ///   der Proxy-Socket verletzt die [`MountPolicy`],
    /// - `SANDBOX_002`: `bwrap` ist älter als `sandbox.min_bwrap_version`,
    /// - `SANDBOX_005`: das Projektverzeichnis fehlt, ist keines oder ist bei
    ///   `rw` nicht beschreibbar,
    /// - `SANDBOX_011`: Proxy-Socket, CA-Zertifikat, CA-Bundle oder Shim
    ///   fehlen, oder memfds und Pipes ließen sich nicht anlegen.
    fn plan(
        &self,
        profile: &SandboxProfile,
        session: &SessionContext,
    ) -> Result<LaunchPlan, Diagnostic> {
        let policy = MountPolicy::from_paths(&self.paths);
        profile.validate_with(&policy)?;

        let required = Version::parse(&profile.sandbox.min_bwrap_version);
        if self.version < required {
            return Err(Diagnostic::builder(SANDBOX_002, Severity::Blocking)
                .why(format!(
                    "{} is bubblewrap {}; profile {:?} needs at least {}",
                    self.program.display(),
                    self.version,
                    profile.name,
                    profile.sandbox.min_bwrap_version
                ))
                .fix(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
                .build());
        }

        Self::check_work_dir(profile, session)?;
        policy.check_work_dir(&session.work_src)?;
        self.check_proxy_socket(session)?;
        Self::check_regular_file(&session.ca_cert_src, "CA certificate")?;
        Self::check_regular_file(&session.ca_bundle_src, "CA bundle")?;
        Self::check_shim(session)?;

        // Die Deskriptoren, in der Reihenfolge, in der sie vererbt werden:
        // die zwei Pipes, die drei Identitätsdateien, dann eine Maske je
        // Eintrag von `masks_to_render`, jede mit ihrem eigenen memfd.
        let (report_reader, report_writer) = std::io::pipe().map_err(|err| {
            placeholder_diagnostic(format!("cannot create the report pipe: {err}"))
        })?;
        let (status_reader, status_writer) = std::io::pipe().map_err(|err| {
            placeholder_diagnostic(format!("cannot create the status pipe: {err}"))
        })?;
        let identity = profile.identity_files(
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        );
        let passwd = Self::sealed_memfd("humanitl-passwd", identity.passwd.as_bytes())?;
        let group = Self::sealed_memfd("humanitl-group", identity.group.as_bytes())?;
        let hosts = Self::sealed_memfd("humanitl-hosts", identity.hosts.as_bytes())?;
        let present = Self::present_under_work(profile, &session.work_src);
        let masks = profile
            .masks_to_render(Some(&present))
            .iter()
            .map(|_| Self::sealed_memfd(MASK_MEMFD_NAME, b""))
            .collect::<Result<Vec<OwnedFd>, Diagnostic>>()?;

        let (agent_fds, agent_files) = Self::agent_file_fds(session)?;

        let inputs = LaunchInputs {
            masks: MaskFds::Each(masks.iter().map(AsRawFd::as_raw_fd).collect()),
            identity: Some(IdentityFds {
                passwd: passwd.as_raw_fd(),
                group: group.as_raw_fd(),
                hosts: hosts.as_raw_fd(),
            }),
            report_fd: Some(report_writer.as_raw_fd()),
            status_fd: Some(status_writer.as_raw_fd()),
            present_under_work: Some(present),
            agent_files,
        };
        let env = profile.effective_env(session, inputs.report_fd);
        let mut argv = vec![self.program.clone().into_os_string()];
        argv.extend(profile.to_bwrap_args(session, &inputs));

        let mut inherit = vec![
            OwnedFd::from(report_writer),
            OwnedFd::from(status_writer),
            passwd,
            group,
            hosts,
        ];
        inherit.extend(masks);
        inherit.extend(agent_fds);
        let fds = inherit
            .iter()
            .map(|fd| (fd.as_raw_fd(), fd.as_raw_fd()))
            .collect();
        Ok(LaunchPlan {
            argv,
            env,
            fds,
            files: session.files.clone(),
            session: session.session,
            profile: profile.name.clone(),
            program: self.program.clone(),
            once: Mutex::new(Some(LaunchOnce {
                inherit,
                report: report_reader,
                status: status_reader,
            })),
        })
    }

    /// Startet den Plan in einem eigenen Thread und wartet höchstens
    /// [`EARLY_EXIT_WINDOW`] auf ein frühes Ende oder die erste Zeile des
    /// Shim-Berichts.
    ///
    /// # Errors
    ///
    /// - `SANDBOX_001`: `bwrap` ließ sich nicht ausführen,
    /// - `SANDBOX_003`: der Kernel erlaubt keinen Nutzer-Namensraum,
    /// - `SANDBOX_012`: `bwrap` endete innerhalb des Fensters, ohne den Befehl
    ///   gestartet zu haben (mit seiner Fehlerausgabe, wenn sie gesammelt
    ///   wurde), die Leser-Threads ließen sich nicht starten, oder der Plan
    ///   war schon gestartet.
    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic> {
        let once = plan
            .take_once()
            .ok_or_else(|| Self::already_launched(plan))?;
        let LaunchOnce {
            inherit,
            report,
            status,
        } = once;
        let id = SandboxId::new();
        let shared = Arc::new(Shared::default());
        if let Some(sink) = self.output_sink.clone() {
            shared.set_sink(sink);
        }
        let program = self.program.clone();
        let args: Vec<OsString> = plan.argv.iter().skip(1).cloned().collect();
        let stdio = self.stdio;

        let (tx, rx) = mpsc::channel();
        let supervisor_shared = Arc::clone(&shared);
        thread::Builder::new()
            .name(format!("sandbox-{}", plan.profile))
            .spawn(move || supervise(&program, &args, inherit, stdio, &supervisor_shared, &tx))
            .map_err(|err| {
                Diagnostic::builder(SANDBOX_012, Severity::Blocking)
                    .why(format!("cannot start the supervisor thread: {err}"))
                    .build()
            })?;
        let pid = rx.recv().map_err(|_| {
            Diagnostic::builder(SANDBOX_012, Severity::Blocking)
                .why("the supervisor thread ended without starting bwrap")
                .build()
        })??;
        let handle = SandboxHandle::new(id, pid, plan.argv_line(), Arc::clone(&shared));

        // Ohne die Leser gäbe es weder Bericht noch Befund; dann lieber gar
        // keine Sandbox als eine, über die niemand etwas sagen kann.
        let readers = spawn_reader("sandbox-report", Arc::clone(&shared), report, read_report)
            .and_then(|()| {
                spawn_reader("sandbox-status", Arc::clone(&shared), status, read_status)
            });
        if let Err(err) = readers {
            handle.kill();
            return Err(Diagnostic::builder(SANDBOX_012, Severity::Blocking)
                .why(format!("cannot start the reader threads for bwrap: {err}"))
                .build());
        }

        // Ein frühes Ende, oder die erste Zeile des Shims: was zuerst kommt.
        let _ = shared.wait_report(EARLY_EXIT_WINDOW, |report| !report.checks.is_empty());
        if let Some(exit) = handle.try_wait() {
            shared.verdict(exit)?;
        }
        Ok(handle)
    }

    /// Ordnet den Bericht des Shims ([`crate::bridge_env`]) den drei Garantien zu.
    ///
    /// `no_interfaces` ist Garantie 1. Garantie 2 heißt „genau eine Tür", und
    /// das sind zwei Aussagen: `single_socket` zeigt, dass im Dateisystem der
    /// Sandbox kein Unix-Socket liegt außer dem der Bridge (der Shim läuft
    /// dafür vor dem `exec` einen begrenzten Suchlauf), `bridge_listening`
    /// zeigt, dass diese eine Tür offen ist und antwortet. Beide Zeilen
    /// stehen im Befund, und eine davon rot macht die Garantie rot; bis zum
    /// Review vom 2026-09-03 trug `bridge_listening` die Garantie allein und
    /// behauptete damit mehr, als sie belegte. `seccomp_applied` und
    /// `families` zusammen sind Garantie 3. Ohne jede Zeile tragen alle drei
    /// `SANDBOX_013`.
    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult> {
        let report = handle.wait_for_report(self.report_timeout);
        if report.checks.is_empty() {
            let mut evidence = format!(
                "no CHECK line from the shim within {:?}",
                self.report_timeout
            );
            if report.closed {
                evidence.push_str("; the report pipe is closed");
            }
            if report.other_lines > 0 {
                let _ = std::fmt::Write::write_fmt(
                    &mut evidence,
                    format_args!("; {} other line(s) ignored", report.other_lines),
                );
            }
            return IsolationCheck::ALL
                .iter()
                .map(|&check| CheckResult {
                    check,
                    passed: false,
                    evidence: evidence.clone(),
                    diagnostic: Some(
                        Diagnostic::builder(SANDBOX_013, Severity::Blocking)
                            .why(format!("{}: {evidence}", check.as_str()))
                            .build(),
                    ),
                })
                .collect();
        }
        vec![
            check_from(
                &report,
                IsolationCheck::NoNetworkInterface,
                &[CHECK_NO_INTERFACES],
                SANDBOX_014,
            ),
            check_from(
                &report,
                IsolationCheck::SingleSocket,
                &[CHECK_SINGLE_SOCKET, CHECK_BRIDGE_LISTENING],
                SANDBOX_015,
            ),
            check_from(
                &report,
                IsolationCheck::SeccompActive,
                &[CHECK_SECCOMP_APPLIED, CHECK_FAMILIES],
                SANDBOX_016,
            ),
        ]
    }
}

/// Ein Ergebnis aus den Zeilen zu `names`: alle müssen da und `ok` sein.
fn check_from(
    report: &ReportSnapshot,
    check: IsolationCheck,
    names: &[&str],
    code: DiagnosticCode,
) -> CheckResult {
    let mut passed = true;
    let mut evidence = Vec::new();
    for name in names {
        match report.get(name) {
            Some(line) if line.ok => evidence.push(format!("{name} ok: {}", line.evidence)),
            Some(line) => {
                passed = false;
                evidence.push(format!("{name} FAIL: {}", line.evidence));
            }
            None => {
                passed = false;
                evidence.push(format!("{name} not reported"));
            }
        }
    }
    let evidence = evidence.join("; ");
    let diagnostic = (!passed).then(|| {
        Diagnostic::builder(code, Severity::Blocking)
            .why(format!("{}: {evidence}", check.as_str()))
            .build()
    });
    CheckResult {
        check,
        passed,
        evidence,
        diagnostic,
    }
}

/// Der Thread, der `bwrap` startet und auf es wartet; siehe Modulbeschreibung.
fn supervise(
    program: &Path,
    args: &[OsString],
    inherit: Vec<OwnedFd>,
    stdio: StdioMode,
    shared: &Arc<Shared>,
    tx: &Sender<Result<u32, Diagnostic>>,
) {
    let spawned = {
        let _guard = SPAWN_LOCK.lock().unwrap_or_else(PoisonError::into_inner);
        let mut command = BwrapBackend::scrubbed_command(program);
        command.args(args);
        if stdio == StdioMode::Capture {
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            shared.set_capturing();
        }
        for fd in &inherit {
            if let Err(err) = fcntl_setfd(fd.as_fd(), FdFlags::empty()) {
                let _ = tx.send(Err(Diagnostic::builder(SANDBOX_012, Severity::Blocking)
                    .why(format!(
                        "cannot hand descriptor {} to bwrap: {err}",
                        fd.as_raw_fd()
                    ))
                    .build()));
                return;
            }
        }
        let spawned = command.spawn();
        // Die Kopien des Kindes bleiben; die eigenen gehen zu, damit die
        // Pipes enden, wenn die Sandbox endet.
        drop(inherit);
        spawned
    };
    let mut child = match spawned {
        Ok(child) => child,
        Err(err) => {
            let _ = tx.send(Err(Diagnostic::builder(SANDBOX_001, Severity::Blocking)
                .why(format!("cannot execute {}: {err}", program.display()))
                .fix(FixAction::CopyCommand(INSTALL_COMMAND.to_owned()))
                .build()));
            return;
        }
    };
    if let Some(stdout) = child.stdout.take() {
        let target = Arc::clone(shared);
        shared.add_reader(thread::spawn(move || {
            drain(stdout, |chunk| target.append_stdout(chunk));
        }));
    }
    if let Some(stderr) = child.stderr.take() {
        let target = Arc::clone(shared);
        shared.add_reader(thread::spawn(move || {
            drain(stderr, |chunk| target.append_stderr(chunk));
        }));
    }
    let _ = tx.send(Ok(child.id()));

    let status = loop {
        match child.wait() {
            Ok(status) => break status,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            // Ein anderer Fehler beim Warten auf das eigene Kind gibt es
            // nicht; sollte er kommen, gilt die Sandbox als beendet.
            Err(_) => break ExitStatus::default(),
        }
    };
    // Erst die Leser einsammeln, dann den Zuhörer der Ausgabe loslassen, dann
    // den Status setzen. Ein Zuhörer, der bliebe, hielte seinen Kanal offen,
    // und wer auf dessen Ende wartet, wartete für immer — die Ausgabe ist zu
    // Ende, wenn die Pipes zu sind und nicht, wenn jemand das Handle fallen
    // lässt.
    shared.join_readers();
    shared.clear_sink();
    shared.set_exit(status);
}

fn drain(mut source: impl Read, mut sink: impl FnMut(&[u8])) {
    let mut buffer = [0u8; 8192];
    loop {
        match source.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => sink(&buffer[..n]),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => break,
        }
    }
}

fn spawn_reader(
    name: &str,
    shared: Arc<Shared>,
    reader: PipeReader,
    read: fn(PipeReader, &Shared),
) -> std::io::Result<()> {
    let handle = thread::Builder::new()
        .name(name.to_owned())
        .spawn(move || read(reader, &shared))?;
    // Nicht eingesammelt: der Leser endet mit der Pipe, und die endet mit
    // der Sandbox.
    drop(handle);
    Ok(())
}

fn read_report(reader: PipeReader, shared: &Shared) {
    for line in BufReader::new(reader).lines() {
        let Ok(line) = line else {
            break;
        };
        match parse_check_line(&line) {
            Some(check) => shared.push_check(check),
            None => shared.push_other_line(),
        }
    }
    shared.close_report();
}

fn read_status(reader: PipeReader, shared: &Shared) {
    for line in BufReader::new(reader).lines() {
        let Ok(line) = line else {
            break;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if let Some(pid) = value
            .get("child-pid")
            .and_then(serde_json::Value::as_u64)
            .and_then(|pid| u32::try_from(pid).ok())
        {
            shared.set_child_pid(pid);
        }
        if let Some(code) = value
            .get("exit-code")
            .and_then(serde_json::Value::as_i64)
            .and_then(|code| i32::try_from(code).ok())
        {
            shared.set_exit_code(code);
        }
    }
    shared.close_status();
}

/// Ob eine `bwrap`-Fehlerausgabe „keine Nutzer-Namensräume" bedeutet.
#[must_use]
pub fn is_userns_failure(stderr: &str) -> bool {
    [
        "uid map",
        "gid map",
        "new namespace",
        "user namespace",
        "unprivileged_userns",
        "No permissions to creat",
    ]
    .iter()
    .any(|needle| stderr.contains(needle))
}

/// Der Befund `SANDBOX_003` mit dem Befehl, der die Sperre aufhebt.
pub(crate) fn userns_diagnostic(stderr: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_003, Severity::Blocking)
        .why(format!(
            "unprivileged user namespaces are disabled (Ubuntu: AppArmor userns restriction, elsewhere kernel.unprivileged_userns_clone); bwrap said: {stderr}"
        ))
        .fix(FixAction::CopyCommand(USERNS_SYSCTL_COMMAND.to_owned()))
        .docs(USERNS_DOCS_URL)
        .build()
}

fn work_dir_diagnostic(why: String) -> Diagnostic {
    Diagnostic::builder(SANDBOX_005, Severity::Blocking)
        .why(why)
        .build()
}

fn placeholder_diagnostic(why: String) -> Diagnostic {
    Diagnostic::builder(SANDBOX_011, Severity::Blocking)
        .why(why)
        .build()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Read;
    use std::path::Path;

    use humanitl_config::Env;

    use super::{BwrapBackend, MIN_BWRAP_VERSION, Version, is_userns_failure};

    #[test]
    fn versions_compare_by_number_not_by_text() {
        assert_eq!(Version::parse("0.11.2"), Version(0, 11, 2));
        assert_eq!(Version::parse("bubblewrap 0.8"), Version(0, 8, 0));
        assert!(Version::parse("0.11.2") > MIN_BWRAP_VERSION);
        assert!(Version::parse("0.9") < Version::parse("0.10"));
        assert_eq!(Version::parse("nonsense"), Version(0, 0, 0));
        assert_eq!(Version(0, 8, 0).to_string(), "0.8.0");
    }

    #[test]
    fn the_userns_patterns_match_bwraps_messages_only() {
        for message in [
            "bwrap: setting up uid map: Permission denied",
            "bwrap: Creating new namespace failed, likely because the kernel does not support user namespaces.",
            "bwrap: No permissions to creating new namespace, likely because the kernel does not allow non-privileged user namespaces.",
        ] {
            assert!(is_userns_failure(message), "{message}");
        }
        for message in [
            "bwrap: Can't find source path /nonexistent: No such file or directory",
            "bwrap: execvp /nonexistent: No such file or directory",
            "",
        ] {
            assert!(!is_userns_failure(message), "{message}");
        }
    }

    #[test]
    fn find_program_reads_path_from_the_given_env_only() {
        let err = BwrapBackend::find_program(&Env::from_pairs([("PATH", "/nonexistent")]))
            .expect_err("nothing there");
        assert_eq!(err.code.as_str(), "SANDBOX_001");
        assert!(err.why.contains("/nonexistent"), "{}", err.why);
        assert!(err.fix.is_some());

        let dir = tempfile::tempdir().expect("tempdir");
        let fake = dir.path().join("bwrap");
        std::fs::write(&fake, b"#!/bin/sh\necho bubblewrap 0.0.1\n").expect("write");
        let mut perms = std::fs::metadata(&fake).expect("meta").permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&fake, perms).expect("chmod");
        let found = BwrapBackend::find_program(&Env::from_pairs([(
            "PATH",
            format!("/nonexistent:{}", dir.path().display()),
        )]))
        .expect("found in the second entry");
        assert_eq!(found, fake);
        assert_eq!(
            BwrapBackend::query_version(&found).expect("runs"),
            Version(0, 0, 1)
        );
        let err = BwrapBackend::detect_program(
            humanitl_config::Paths::new(Env::from_pairs([("HOME", "/home/u")])),
            found,
        )
        .expect_err("0.0.1 is too old");
        assert_eq!(err.code.as_str(), "SANDBOX_002");
        assert!(err.why.contains("0.0.1"), "{}", err.why);
    }

    #[test]
    fn a_scrubbed_command_carries_no_environment() {
        let command = BwrapBackend::scrubbed_command(Path::new("/usr/bin/env"));
        assert_eq!(command.get_envs().count(), 0);
        let output = BwrapBackend::scrubbed_command(Path::new("/usr/bin/env"))
            .output()
            .expect("env runs");
        assert!(
            output.stdout.is_empty(),
            "the child saw: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }

    /// Ein memfd mit Inhalt liest sich von vorn, und danach lässt es sich
    /// weder beschreiben noch vergrößern.
    #[test]
    fn a_sealed_memfd_reads_its_content_from_the_start_and_refuses_writes() {
        let fd = BwrapBackend::sealed_memfd("humanitl-test", b"agent:x:1:1::/home/agent:/bin/sh\n")
            .expect("memfd");
        let mut file = std::fs::File::from(fd);
        let mut text = String::new();
        file.read_to_string(&mut text).expect("read");
        assert_eq!(text, "agent:x:1:1::/home/agent:/bin/sh\n");
        let err = std::io::Write::write_all(&mut file, b"more").expect_err("sealed");
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
        assert_eq!(file.metadata().expect("meta").len(), 33);

        let empty = BwrapBackend::sealed_memfd("humanitl-mask", b"").expect("memfd");
        assert_eq!(
            std::fs::File::from(empty).metadata().expect("meta").len(),
            0
        );
    }
}
