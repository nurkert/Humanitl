//! Das `bwrap`-Backend (ADR-002, HUM-011, HUM-013).
//!
//! [`BwrapBackend`] findet `bwrap`, prüft Version und Nutzer-Namensräume,
//! baut aus Profil und Sitzung den [`LaunchPlan`] und startet ihn. Drei Dinge
//! sind hier anders als in einem gewöhnlichen `Command::spawn`, und jedes hat
//! einen Grund:
//!
//! - **Leere Umgebung.** `bwrap` bekommt keine einzige Variable des Hosts.
//!   `--clearenv` räumt nur die Umgebung des Befehls in der Sandbox auf.
//!   Ohne `--as-pid-1` blieb `bwrap` als PID 1 des Namensraums stehen, und
//!   `/proc/1/environ` zeigte Tokens, `DISPLAY` und
//!   `DBUS_SESSION_BUS_ADDRESS` des Nutzers (ESC-2-Befund, CONVENTIONS 4.11).
//!   Seit HUM-203 ist der Shim PID 1 und nicht dumpable; die leere Umgebung
//!   bleibt trotzdem, für die Proben des Doctors, die ohne `--as-pid-1`
//!   starten, und damit ein künftiger Fehler im Prozessmodell nichts preisgibt.
//! - **Geerbte Deskriptoren, keine Umnummerierung.** Die Masken und die drei
//!   Identitätsdateien kommen aus versiegelten memfds, einer je Datei
//!   (`bwrap` schließt jeden nach dem Lesen); der Bericht des Shims und der
//!   Status von `bwrap` über je eine Pipe; dazu die geöffneten Verzeichnisse
//!   (HUM-263). Sie werden unter ihrer eigenen Nummer vererbt, und
//!   `FD_CLOEXEC` fällt nur im Kind, zwischen `fork` und `exec`
//!   (`inherit.rs`). Im eigenen Prozess bleibt es die ganze Zeit
//!   stehen, sodass kein anderer Start im selben Augenblick sie miterbt.
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
use std::io::{BufRead, BufReader, PipeReader, Read};
#[cfg(target_os = "linux")]
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStringExt as _;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use humanitl_config::{Env, Paths, WorkMode};
use humanitl_core::diagnostics::codes::{
    SANDBOX_001, SANDBOX_002, SANDBOX_003, SANDBOX_005, SANDBOX_006, SANDBOX_011, SANDBOX_012,
    SANDBOX_013, SANDBOX_014, SANDBOX_015, SANDBOX_016, SANDBOX_030, TERM_002,
};
use humanitl_core::ids::SandboxId;
use humanitl_core::{Diagnostic, DiagnosticCode, FixAction, Severity};
use rustix::fs::{Access, access};
#[cfg(target_os = "linux")]
use rustix::fs::{MemfdFlags, SealFlags, fcntl_add_seals, memfd_create};
use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};

use crate::bridge_env::{
    CHECK_BRIDGE_LISTENING, CHECK_FAMILIES, CHECK_NO_INTERFACES, CHECK_SECCOMP_APPLIED,
    CHECK_SINGLE_SOCKET, parse_check_line, parse_exec_line,
};
use crate::bwrap_args::{IdentityFds, LaunchInputs, MaskFds, SourceFd};
use crate::handle::{OutputSink, ReportSnapshot, SandboxHandle, Shared};
use crate::inherit::{HOOK_ERRNO, above_stdio, inherit_in_child};
use crate::launcher::{
    CheckResult, IsolationCheck, LaunchOnce, LaunchPlan, SandboxBackend, StdioMode,
};
use crate::os_release::install_command;
use crate::pinned::{PinnedSource, SourceKind};
use crate::profile::{MountPolicy, SandboxProfile, SessionContext, WORK_DST, normalize};

/// Die kleinste `bwrap`-Fassung, mit der der Launcher arbeitet.
///
/// `--disable-userns` gibt es seit 0.6, `--json-status-fd` und
/// `--ro-bind-data` länger; 0.8 ist, was Debian 12 und Ubuntu 24.04
/// ausliefern, und was das Profil als Untergrenze nennt.
pub const MIN_BWRAP_VERSION: Version = Version(0, 8, 0);

/// Ab dieser `bwrap`-Fassung gibt es `--bind-fd` und `--ro-bind-fd`
/// (bubblewrap 0.10.0, eingeführt gegen CVE-2024-42472 in Flatpak).
///
/// Darunter bindet der Launcher einen geöffneten Ordner über
/// `/proc/self/fd/N` (HUM-263, [`SourceFd::ProcSelf`]). Beides hängt genau
/// das Verzeichnis ein, das geprüft wurde; der Unterschied ist, wer den
/// Deskriptor schließt: mit `--bind-fd` `bwrap` selbst, sonst der Shim.
pub const BIND_FD_VERSION: Version = Version(0, 10, 0);

/// Wie lange [`BwrapBackend::launch`] nach dem Start auf ein frühes Ende von
/// `bwrap` wartet, bevor es die Sandbox als laufend meldet (HUM-011: 500 ms).
/// Die erste Zeile des Shim-Berichts beendet das Warten früher: sie kommt
/// erst, wenn der Befehl läuft.
pub const EARLY_EXIT_WINDOW: Duration = Duration::from_millis(500);

/// Wie lange [`SandboxBackend::isolation_check`] auf den Bericht des Shims wartet.
pub const REPORT_TIMEOUT: Duration = Duration::from_secs(5);

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
    /// Nur lesbare Binds, die der Aufrufer zur Sitzung beiträgt, als
    /// `(Quelle, Ziel)`; siehe [`BwrapBackend::with_read_only_bind`].
    read_only_binds: Vec<(PathBuf, PathBuf)>,
    /// Der aufgelöste Ort, an dem der Aufrufer den Projektordner geprüft hat;
    /// siehe [`BwrapBackend::with_approved_work_dir`].
    approved_work_dir: Option<PathBuf>,
    /// Die Schicht einer Wegwerf-Sandbox; siehe [`BwrapBackend::with_overlay`].
    overlay: Option<crate::disposable::OverlayDirs>,
    /// Gerät und Inode des Projektordners, den der Aufrufer schon geöffnet
    /// hält; siehe [`BwrapBackend::with_expected_work_inode`].
    expected_work_inode: Option<(u64, u64)>,
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
                .fix(FixAction::CopyCommand(install_command().to_owned()))
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
            read_only_binds: Vec::new(),
            approved_work_dir: None,
            overlay: None,
            expected_work_inode: None,
        }
    }

    /// Ohne Prüfung, mit `bwrap` aus `PATH`: der Rückfall, wenn
    /// [`Self::detect`] scheitert und trotzdem ein Plan gezeigt werden soll
    /// (HUM-323, dieselbe Form wie [`crate::SeatbeltBackend::fallback`]).
    #[must_use]
    pub fn fallback(paths: Paths) -> Self {
        Self::unchecked("bwrap", MIN_BWRAP_VERSION, paths)
    }

    /// Startet die Sandbox als Wegwerf-Sandbox (HUM-350): das
    /// Projektverzeichnis nur lesbar als untere Schicht, `dirs` als obere
    /// Schicht und Arbeitsverzeichnis des Overlays, `mounts.extra_rw` nur
    /// lesbar. Die Verzeichnisse legt der Aufrufer an
    /// (`crate::disposable::Layer::create`).
    #[must_use]
    pub fn with_overlay(mut self, dirs: crate::disposable::OverlayDirs) -> Self {
        self.overlay = Some(dirs);
        self
    }

    /// Bindet den Projektordner nur, wenn der hier geöffnete Ordner Gerät und
    /// Inode `(dev, ino)` trägt (HUM-350): Der Aufrufer hält den Ordner schon
    /// offen und schreibt später durch diesen Deskriptor; ein Umbenennen und
    /// ein neuer Ordner unter demselben Pfad dazwischen hängte sonst einen
    /// anderen Ordner ein als den, in den übernommen wird.
    #[must_use]
    pub const fn with_expected_work_inode(mut self, dev: u64, ino: u64) -> Self {
        self.expected_work_inode = Some((dev, ino));
        self
    }

    /// Die Schicht, wenn dies eine Wegwerf-Sandbox ist.
    #[must_use]
    pub const fn overlay(&self) -> Option<&crate::disposable::OverlayDirs> {
        self.overlay.as_ref()
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

    /// Ein nur lesbarer Bind von `src` nach `dst`, den der Aufrufer zur
    /// Sitzung beiträgt, nicht das Profil: das Testverzeichnis der
    /// Escape-Tests (`--tests-dir`).
    ///
    /// Er geht denselben Weg wie die Einträge aus `mounts.extra_ro` (HUM-263):
    /// [`SandboxBackend::plan`] öffnet die Quelle einmal, prüft den Ort ihres
    /// Deskriptors mit [`MountPolicy::check_session_dir`] und bindet den
    /// Deskriptor, hinter den Erweiterungen des Profils und vor dem
    /// Projektordner. Geprüft wird wie beim Projektordner, denn auch diese
    /// Quelle kommt aus dem Aufruf und darf unter dem Heimatverzeichnis
    /// liegen; das Heimatverzeichnis selbst und die geschützten Bäume nicht.
    #[must_use]
    pub fn with_read_only_bind(mut self, src: impl Into<PathBuf>, dst: impl Into<PathBuf>) -> Self {
        self.read_only_binds.push((src.into(), dst.into()));
        self
    }

    /// Der aufgelöste Ort, an dem der Aufrufer den Projektordner geprüft und
    /// freigegeben hat (HUM-294).
    ///
    /// [`SandboxBackend::plan`] öffnet den Projektordner einmal (HUM-263) und
    /// bindet ihn nur, wenn der Ort des Deskriptors genau dieser ist. Der
    /// Aufrufer prüft mehr als die Politik hier, etwa ob der Ordner unter dem
    /// Heimatverzeichnis liegt oder erklärt ist; ersetzt ein Verweis zwischen
    /// seiner Prüfung und dem Öffnen einen Teil des Pfades, führte derselbe
    /// Text sonst an einen Ort, den nur die Denylist gesehen hat. Ohne Angabe
    /// gilt allein die Politik, wie bisher.
    #[must_use]
    pub fn with_approved_work_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.approved_work_dir = Some(dir.into());
        self
    }

    /// Der freigegebene Ort des Projektordners, falls der Aufrufer einen
    /// genannt hat ([`BwrapBackend::with_approved_work_dir`]).
    #[must_use]
    pub fn approved_work_dir(&self) -> Option<&Path> {
        self.approved_work_dir.as_deref()
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
            .fix(FixAction::CopyCommand(install_command().to_owned()))
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
                    .fix(FixAction::CopyCommand(install_command().to_owned()))
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

    /// Die Bedingungen einer Wegwerf-Sandbox (HUM-350): `bwrap` kennt
    /// `--overlay`, und die Sandbox sieht das Projekt schreibbar, sonst gäbe
    /// es nichts zu verwerfen.
    ///
    /// # Errors
    ///
    /// `SANDBOX_030`, wenn eine fehlt.
    fn check_overlay(
        &self,
        profile: &SandboxProfile,
        session: &SessionContext,
    ) -> Result<(), Diagnostic> {
        if self.overlay.is_none() {
            return Ok(());
        }
        if self.version < crate::disposable::MIN_OVERLAY_VERSION {
            return Err(Diagnostic::builder(SANDBOX_030, Severity::Blocking)
                .why(format!(
                    "{} is bubblewrap {}; a disposable sandbox needs --overlay, which came \
                     with bubblewrap {}",
                    self.program.display(),
                    self.version,
                    crate::disposable::MIN_OVERLAY_VERSION
                ))
                .fix(FixAction::CopyCommand(install_command().to_owned()))
                .build());
        }
        if profile.effective_work_mode(session.work_mode) != WorkMode::Rw {
            return Err(Diagnostic::builder(SANDBOX_030, Severity::Blocking)
                .why(
                    "a disposable sandbox shows the agent a writable project folder and keeps \
                     its changes apart, but the project folder is mounted read-only here \
                     (sandbox.work_mode or the profile)",
                )
                .fix(FixAction::ChangeSetting {
                    key: "sandbox.work_mode".to_owned(),
                    value: "rw".to_owned(),
                })
                .build());
        }
        Ok(())
    }

    pub(crate) fn check_work_dir(
        profile: &SandboxProfile,
        session: &SessionContext,
    ) -> Result<(), Diagnostic> {
        let work = session.work_src.as_path();
        let shown = work.display();
        if !work.is_absolute() {
            return Err(work_dir_diagnostic(format!(
                "the project directory {shown} is not an absolute path"
            )));
        }
        let meta = std::fs::metadata(work).map_err(|err| {
            work_dir_diagnostic(format!(
                "the project directory {shown} does not exist: {err}"
            ))
        })?;
        if !meta.is_dir() {
            return Err(work_dir_diagnostic(format!(
                "the project directory {shown} is not a directory"
            )));
        }
        if profile.effective_work_mode(session.work_mode) == WorkMode::Rw
            && access(work, Access::WRITE_OK).is_err()
        {
            return Err(Diagnostic::builder(SANDBOX_005, Severity::Blocking)
                .why(format!(
                    "the project directory {shown} is not writable, but sandbox.work_mode is rw"
                ))
                .fix(FixAction::RemountReadOnly(work.to_path_buf()))
                .build());
        }
        Ok(())
    }

    /// Öffnet eine Mount-Quelle einmal und hält sie als Deskriptor (HUM-263).
    ///
    /// `whence` nennt die Quelle im `why`, zum Beispiel `sandbox.work_dir`.
    ///
    /// # Errors
    ///
    /// `SANDBOX_006`, wenn die Quelle fehlt, nicht von der verlangten Art ist
    /// oder zwischen Auflösen und Öffnen getauscht wurde.
    fn pin(source: &Path, kind: SourceKind, whence: &str) -> Result<PinnedSource, Diagnostic> {
        PinnedSource::open(source, kind).map_err(|why| {
            Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                .why(format!(
                    "{whence}: {why}; a mount source is opened once and mounted as opened, \
                     so a source that cannot be opened is not mounted"
                ))
                .build()
        })
    }

    /// Wie `bwrap` einen geöffneten Deskriptor bekommt, nach seiner Fassung.
    fn source_fd(&self, pinned: &PinnedSource) -> SourceFd {
        if self.version >= BIND_FD_VERSION {
            SourceFd::BindFd(pinned.raw_fd())
        } else {
            SourceFd::ProcSelf(pinned.raw_fd())
        }
    }

    fn check_proxy_socket(&self, session: &SessionContext) -> Result<(), Diagnostic> {
        Self::check_proxy_socket_in(&self.paths, session)
    }

    /// [`Self::check_proxy_socket`] for any backend: the socket of these
    /// paths, 0600, from the proxy directory and nowhere else (HUM-323).
    pub(crate) fn check_proxy_socket_in(
        paths: &Paths,
        session: &SessionContext,
    ) -> Result<(), Diagnostic> {
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
        let dir = normalize(&paths.proxy_socket_dir());
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

    pub(crate) fn check_regular_file(path: &Path, what: &str) -> Result<(), Diagnostic> {
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

    pub(crate) fn check_shim(session: &SessionContext) -> Result<(), Diagnostic> {
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

    /// Die `tmpfs`-Pfade unter `/work`, über denen **kein** `tmpfs` liegt
    /// (HUM-043).
    ///
    /// **Die Entscheidung, die HUM-043 offen ließ.** `bwrap` hängt nur über
    /// einen Mountpoint ein, den es gibt, und [`Self::present_under_work`]
    /// rendert ein `--tmpfs` unter `/work` deshalb nur für einen Pfad, den es
    /// auf dem Host als Verzeichnis gibt: Sonst legte `bwrap` ihn selbst an,
    /// auf `rw` als leeres Verzeichnis im Projekt des Nutzers, auf `ro` gar
    /// nicht — dort scheiterte der Start mit `EROFS`. Daraus folgt eine Lücke:
    /// Ohne `.git/hooks` auf dem Host liegt kein `tmpfs` darüber, und ein
    /// `pre-commit`, den der Agent schreibt, landet im Projekt.
    ///
    /// Von den beiden Wegen, die das Issue zur Wahl stellte, nimmt Humanitl den
    /// zweiten: **melden statt anlegen.** Drei Gründe:
    ///
    /// - Ein `.idea/` oder `.vscode/`, das Humanitl erfindet, lässt die
    ///   Werkzeugkette des Nutzers glauben, das Projekt sei dafür eingerichtet;
    ///   ein `.git/` in einem Verzeichnis ohne Repository bringt Git aus dem
    ///   Tritt, das dann ein kaputtes Repository sieht statt keines.
    /// - Die Invariante bleibt einfach: Der Daemon schreibt nicht in das
    ///   Projekt des Nutzers, und zwar auch kein leeres Verzeichnis. Seit
    ///   HUM-037 ist das geprüft (`opencode_adapter.rs`,
    ///   `the_briefing_leaves_the_project_directory_byte_identical`).
    /// - Ein Werkzeug, das erst ein `.git/` anlegt und dann behauptet, es habe
    ///   nichts angefasst, wäre keines.
    ///
    /// Die Lücke bleibt damit offen, aber sie bleibt auch sichtbar, und das ist
    /// genau die Bauweise dieses Kanals (BACKLOG.md 4.2): Was der Agent in ein
    /// Verzeichnis schreibt, das es vorher nicht gab, ist im Diff des Laufs
    /// eine **neue** Datei und steht als solche in der Zusammenfassung. Die
    /// Liste hier geht denselben Weg und nennt die Pfade, über denen die Maske
    /// diesmal nicht lag.
    ///
    /// Die Liste umfasst **beide** Arten von Überdeckung: die `tmpfs` aus
    /// `mounts.tmpfs` und die Masken aus [`SandboxProfile::effective_masked_files`].
    /// Für eine fehlende Maske gilt dasselbe wie für ein fehlendes Verzeichnis:
    /// Ohne die Datei auf dem Host rendert der Launcher kein `--ro-bind-data`,
    /// und ein `.pre-commit-config.yaml`, das der Agent neu anlegt, läuft beim
    /// nächsten Commit.
    ///
    /// Die Pfade kommen **relativ zum Projektverzeichnis** zurück, in demselben
    /// Raum wie die Einträge des Schnappschusses; nur so lässt sich eine
    /// Änderung einem ungeschützten Pfad zuordnen
    /// (`summary::SessionSummary::set_unprotected`).
    pub(crate) fn unprotected_under_work(
        profile: &SandboxProfile,
        present: &BTreeSet<PathBuf>,
    ) -> Vec<PathBuf> {
        let work_dst = profile.mounts.work.dst.as_path();
        profile
            .mounts
            .tmpfs
            .iter()
            .cloned()
            .chain(profile.effective_masked_files())
            .filter(|path| !present.contains(path))
            .filter_map(|path| {
                path.strip_prefix(work_dst)
                    .ok()
                    .filter(|rel| !rel.as_os_str().is_empty())
                    .map(Path::to_path_buf)
            })
            .collect()
    }

    /// Die Pfade unter `/work` (Sandbox-Sicht), die es im Projekt gibt: ein
    /// Verzeichnis je `mounts.tmpfs` darunter, eine Datei je Maske.
    pub(crate) fn present_under_work(
        profile: &SandboxProfile,
        work_src: &Path,
    ) -> BTreeSet<PathBuf> {
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
    #[cfg(target_os = "linux")]
    fn sealed_memfd(name: &str, content: &[u8]) -> Result<OwnedFd, Diagnostic> {
        let fd = memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)
            .map_err(|err| placeholder_diagnostic(format!("cannot create memfd {name}: {err}")))?;
        let mut file = File::from(fd);
        file.write_all(content)
            .and_then(|()| file.seek(SeekFrom::Start(0)))
            .map_err(|err| placeholder_diagnostic(format!("cannot fill memfd {name}: {err}")))?;
        // Nie auf 0, 1 oder 2: Dort ersetzte die stdio des Kindes ihn, bevor
        // er vererbt wird (`inherit.rs`).
        let fd = above_stdio(OwnedFd::from(file))
            .map_err(|err| placeholder_diagnostic(format!("cannot place memfd {name}: {err}")))?;
        fcntl_add_seals(
            &fd,
            SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
        )
        .map_err(|err| placeholder_diagnostic(format!("cannot seal memfd {name}: {err}")))?;
        Ok(fd)
    }

    /// Ohne `memfd_create(2)` gibt es keine versiegelte Datei; bwrap läuft
    /// ohnehin nur unter Linux (HUM-312).
    #[cfg(not(target_os = "linux"))]
    fn sealed_memfd(name: &str, _content: &[u8]) -> Result<OwnedFd, Diagnostic> {
        Err(placeholder_diagnostic(format!(
            "cannot create memfd {name}: this system has no memfd_create"
        )))
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

impl BwrapBackend {
    /// Eine Pipe, deren Schreibseite `bwrap` erbt: Sie liegt deshalb über den
    /// Standardströmen (`inherit.rs`). `name` steht im Befund.
    pub(crate) fn inherited_pipe(name: &str) -> Result<(PipeReader, OwnedFd), Diagnostic> {
        let (reader, writer) = std::io::pipe().map_err(|err| {
            placeholder_diagnostic(format!("cannot create the {name} pipe: {err}"))
        })?;
        let writer = above_stdio(OwnedFd::from(writer)).map_err(|err| {
            placeholder_diagnostic(format!("cannot place the {name} pipe: {err}"))
        })?;
        Ok((reader, writer))
    }

    /// The door for further shells (HUM-299) as `(daemon's end, shim's
    /// end)`, like the controlling terminal only at the daemon's own
    /// pseudo-terminal: a sandbox that runs without the daemon has nobody to
    /// ask for a shell. The shim's end lies above the standard streams
    /// (`inherit.rs`).
    fn shell_door(&self) -> Result<(Option<OwnedFd>, Option<OwnedFd>), Diagnostic> {
        if self.stdio.pty_size().is_none() {
            return Ok((None, None));
        }
        let (daemon_end, shim_end) = crate::shell::pair().map_err(|err| {
            placeholder_diagnostic(format!("cannot create the door for further shells: {err}"))
        })?;
        let shim_end = above_stdio(shim_end).map_err(|err| {
            placeholder_diagnostic(format!("cannot place the door for further shells: {err}"))
        })?;
        Ok((Some(daemon_end), Some(shim_end)))
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
    ///   der Proxy-Socket verletzt die [`MountPolicy`], oder das
    ///   Projektverzeichnis oder ein Eintrag aus `mounts.extra_ro` und
    ///   `mounts.extra_rw` lässt sich nicht als Deskriptor öffnen, fehlt oder
    ///   wurde beim Öffnen getauscht (HUM-263),
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
                .fix(FixAction::CopyCommand(install_command().to_owned()))
                .build());
        }

        self.check_overlay(profile, session)?;
        Self::check_work_dir(profile, session)?;
        policy.check_work_dir(&session.work_src)?;
        // Ab hier zählt der Deskriptor, nicht mehr der Pfad (HUM-263): Die
        // Politik prüft den Ort, an dem das Geöffnete liegt, und `bwrap`
        // bindet genau diesen Deskriptor. Ein Tausch des Pfades zwischen hier
        // und dem Einhängen erreicht die Sandbox nicht mehr. Die Prüfung oben
        // bleibt für den geschriebenen Pfad und für die genaueren Befunde.
        let pinned = PinnedMounts::open(self, &policy, profile, session)?;
        self.check_proxy_socket(session)?;
        Self::check_regular_file(&session.ca_cert_src, "CA certificate")?;
        Self::check_regular_file(&session.ca_bundle_src, "CA bundle")?;
        Self::check_shim(session)?;

        // Die Deskriptoren, in der Reihenfolge, in der sie vererbt werden:
        // die zwei Pipes, die drei Identitätsdateien, dann eine Maske je
        // Eintrag von `masks_to_render`, jede mit ihrem eigenen memfd, die
        // Adapter-Dateien und zuletzt die geöffneten Verzeichnisse (HUM-263).
        let (report_reader, report_writer) = Self::inherited_pipe("report")?;
        let (status_reader, status_writer) = Self::inherited_pipe("status")?;
        let identity = profile.identity_files(
            rustix::process::getuid().as_raw(),
            rustix::process::getgid().as_raw(),
        );
        let passwd = Self::sealed_memfd("humanitl-passwd", identity.passwd.as_bytes())?;
        let group = Self::sealed_memfd("humanitl-group", identity.group.as_bytes())?;
        let hosts = Self::sealed_memfd("humanitl-hosts", identity.hosts.as_bytes())?;
        let mut warnings = profile.unmask_diagnostics();
        // Über den Deskriptor, nicht über den Pfad: Gerendert werden Masken
        // für das Verzeichnis, das eingehängt wird.
        let present = Self::present_under_work(profile, &pinned.work.proc_path());
        let unprotected = Self::unprotected_under_work(profile, &present);
        if let Some(diagnostic) = crate::worktree::resolution_diagnostic() {
            warnings.push(diagnostic);
        }
        let masks = profile
            .masks_to_render(Some(&present))
            .iter()
            .map(|_| Self::sealed_memfd(MASK_MEMFD_NAME, b""))
            .collect::<Result<Vec<OwnedFd>, Diagnostic>>()?;

        let (agent_fds, agent_files) = Self::agent_file_fds(session)?;
        let (work_dir, read_only, writable) = pinned.sources(self);
        let (shell_door, shim_door) = self.shell_door()?;

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
            work_dir: Some(work_dir),
            extra_ro: read_only,
            extra_rw: writable,
            session_ro: pinned.session_sources(self),
            agent_program: pinned.agent.as_ref().map(|agent| self.source_fd(agent)),
            // Only at the daemon's own pseudo-terminal (HUM-280). Inherited
            // descriptors may be the user's terminal, and that one must never
            // become a controlling terminal inside the sandbox (`TIOCSTI`).
            controlling_terminal: self.stdio.pty_size().is_some(),
            control_fd: shim_door.as_ref().map(AsRawFd::as_raw_fd),
            overlay: self.overlay.clone(),
        };
        let env = profile.effective_env(session, inputs.report_fd);
        let mut argv = vec![self.program.clone().into_os_string()];
        argv.extend(profile.to_bwrap_args(session, &inputs));

        let mut inherit = vec![report_writer, status_writer, passwd, group, hosts];
        inherit.extend(masks);
        inherit.extend(agent_fds);
        inherit.extend(pinned.into_fds());
        inherit.extend(shim_door);
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
            warnings,
            unprotected,
            env_on_program: false,
            program: self.program.clone(),
            once: Mutex::new(Some(LaunchOnce {
                inherit,
                report: report_reader,
                status: status_reader,
                shells: shell_door.map(crate::shell::DoorSeed::Socket),
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
    ///   wurde), die Leser-Threads ließen sich nicht starten, die geerbten
    ///   Deskriptoren ließen sich nicht übergeben (HUM-263), oder der Plan
    ///   war schon gestartet.
    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic> {
        launch_plan(&self.program, plan, self.stdio, self.output_sink.clone())
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
        isolation_from_report(handle, self.report_timeout)
    }
}

/// The three guarantees from the shim's report, for every backend whose shim
/// writes the same five `CHECK` lines (HUM-323); see
/// [`BwrapBackend::isolation_check`](SandboxBackend::isolation_check).
pub(crate) fn isolation_from_report(handle: &SandboxHandle, timeout: Duration) -> Vec<CheckResult> {
    let report = handle.wait_for_report(timeout);
    if report.checks.is_empty() {
        let mut evidence = format!("no CHECK line from the shim within {timeout:?}");
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

/// Starts a plan: the supervisor thread, the readers of report and status,
/// and the wait for the first line or an early end. The same for every
/// backend that starts one program with inherited descriptors (bwrap,
/// `sandbox-exec`, HUM-323).
///
/// # Errors
///
/// As [`SandboxBackend::launch`].
pub(crate) fn launch_plan(
    program: &Path,
    plan: &LaunchPlan,
    stdio: StdioMode,
    output_sink: Option<OutputSink>,
) -> Result<SandboxHandle, Diagnostic> {
    let once = plan
        .take_once()
        .ok_or_else(|| BwrapBackend::already_launched(plan))?;
    let LaunchOnce {
        inherit,
        report,
        status,
        shells,
    } = once;
    let id = SandboxId::new();
    let shared = Arc::new(Shared::default());
    if let Some(sink) = output_sink {
        shared.set_sink(sink);
    }
    let program = program.to_path_buf();
    let args: Vec<OsString> = plan.argv.iter().skip(1).cloned().collect();
    // bwrap carries the environment in `--setenv` and starts with an
    // empty one; `sandbox-exec` hands its own environment to the command
    // (HUM-323).
    let env = if plan.env_on_program {
        plan.env.clone()
    } else {
        Vec::new()
    };

    let (tx, rx) = mpsc::channel();
    let supervisor_shared = Arc::clone(&shared);
    thread::Builder::new()
        .name(format!("sandbox-{}", plan.profile))
        .spawn(move || {
            supervise(
                &program,
                &args,
                &env,
                inherit,
                stdio,
                &supervisor_shared,
                &tx,
            );
        })
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
    // The door opens after `bwrap` holds its end, and a door that cannot
    // open costs the further shells, never the session (HUM-299).
    // `SandboxHandle::open_shell` then says that there is no door.
    if let Some(seed) = shells
        && let Ok(door) = seed.open()
    {
        shared.set_shells(door);
    }

    // Ohne die Leser gäbe es weder Bericht noch Befund; dann lieber gar
    // keine Sandbox als eine, über die niemand etwas sagen kann.
    let readers = spawn_reader("sandbox-report", Arc::clone(&shared), report, read_report)
        .and_then(|()| spawn_reader("sandbox-status", Arc::clone(&shared), status, read_status));
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

/// Die Quellen eines Plans, die als Deskriptor gebunden werden (HUM-263):
/// der Projektordner und die Einträge aus `mounts.extra_ro` und
/// `mounts.extra_rw`, jede geöffnet und am Ort ihres Deskriptors geprüft.
struct PinnedMounts {
    work: PinnedSource,
    read_only: Vec<PinnedSource>,
    writable: Vec<PinnedSource>,
    /// Die Binds aus [`BwrapBackend::with_read_only_bind`], mit ihrem Ziel.
    session: Vec<(PinnedSource, PathBuf)>,
    /// The agent program, opened and checked once (HUM-278).
    agent: Option<PinnedSource>,
}

impl PinnedMounts {
    /// Öffnet und prüft alle Quellen.
    ///
    /// # Errors
    ///
    /// `SANDBOX_006` aus [`BwrapBackend::pin`], aus
    /// [`MountPolicy::check_work_dir`] oder aus [`MountPolicy::check`].
    fn open(
        backend: &BwrapBackend,
        policy: &MountPolicy,
        profile: &SandboxProfile,
        session: &SessionContext,
    ) -> Result<Self, Diagnostic> {
        let work = BwrapBackend::pin(
            &session.work_src,
            SourceKind::Directory,
            "the project directory",
        )?;
        policy.check_work_dir(work.resolved())?;
        if let Some(approved) = &backend.approved_work_dir
            && work.resolved() != approved.as_path()
        {
            return Err(Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                .why(format!(
                    "the project directory {} was checked at {}, but once opened it lies at {}; \
                     a symbolic link in its path leads elsewhere now, so it is not mounted",
                    session.work_src.display(),
                    approved.display(),
                    work.resolved().display()
                ))
                .fix(crate::work_dir::choose_folder_fix())
                .build());
        }
        if let Some((dev, ino)) = backend.expected_work_inode {
            let stat = rustix::fs::fstat(work.fd()).map_err(|err| {
                Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                    .why(format!(
                        "the opened project directory {} cannot be inspected ({err})",
                        session.work_src.display()
                    ))
                    .build()
            })?;
            // `st_dev` ist nicht auf jedem System `u64` (macOS: `i32`).
            #[allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]
            let found = (
                u64::try_from(stat.st_dev).ok(),
                u64::try_from(stat.st_ino).ok(),
            );
            if found != (Some(dev), Some(ino)) {
                return Err(Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                    .why(format!(
                        "the project directory {} is not the folder that was opened before the \
                         start (another directory took its path), so it is not mounted",
                        session.work_src.display()
                    ))
                    .fix(crate::work_dir::choose_folder_fix())
                    .build());
            }
        }
        let session_binds = backend
            .read_only_binds
            .iter()
            .map(|(src, dst)| {
                let whence = format!("the read-only bind onto {}", dst.display());
                let pinned = BwrapBackend::pin(src, SourceKind::DirectoryOrFile, &whence)?;
                policy.check_session_dir(pinned.resolved(), &whence)?;
                Ok((pinned, dst.clone()))
            })
            .collect::<Result<Vec<_>, Diagnostic>>()?;
        // The agent program is checked by its owner, mode and ELF header on
        // the opened descriptor, not by the mount denylist: it is one file the
        // daemon resolved itself (HUM-278, docs/SECURITY.md).
        let agent = session
            .agent_program
            .as_ref()
            .map(|program| {
                crate::agent::locate::pin_program(
                    &program.host,
                    &crate::agent::locate::Trust::of_this_process(),
                )
                .map_err(|skip| {
                    Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                        .why(format!(
                            "the agent program {} is no longer bindable: {}; it was checked \
                             when the session was planned and is opened again at the start",
                            program.host.display(),
                            skip.describe()
                        ))
                        .build()
                })
            })
            .transpose()?;
        Ok(Self {
            agent,
            work,
            read_only: Self::open_list(policy, &profile.mounts.extra_ro, "mounts.extra_ro")?,
            writable: Self::open_list(policy, &profile.mounts.extra_rw, "mounts.extra_rw")?,
            session: session_binds,
        })
    }

    fn open_list(
        policy: &MountPolicy,
        sources: &[PathBuf],
        whence: &str,
    ) -> Result<Vec<PinnedSource>, Diagnostic> {
        sources
            .iter()
            .map(|source| {
                let pinned = BwrapBackend::pin(source, SourceKind::DirectoryOrFile, whence)?;
                policy.check(pinned.resolved(), whence)?;
                Ok(pinned)
            })
            .collect()
    }

    /// Wie `bwrap` die Deskriptoren bekommt: Projektordner, `extra_ro`,
    /// `extra_rw`.
    fn sources(&self, backend: &BwrapBackend) -> (SourceFd, Vec<SourceFd>, Vec<SourceFd>) {
        let each = |list: &[PinnedSource]| list.iter().map(|p| backend.source_fd(p)).collect();
        (
            backend.source_fd(&self.work),
            each(&self.read_only),
            each(&self.writable),
        )
    }

    /// Die Binds des Aufrufers, mit ihrem Ziel.
    fn session_sources(&self, backend: &BwrapBackend) -> Vec<(SourceFd, PathBuf)> {
        self.session
            .iter()
            .map(|(pinned, dst)| (backend.source_fd(pinned), dst.clone()))
            .collect()
    }

    /// Die Deskriptoren zum Vererben, in derselben Reihenfolge.
    fn into_fds(self) -> impl Iterator<Item = OwnedFd> {
        std::iter::once(self.work)
            .chain(self.read_only)
            .chain(self.writable)
            .chain(self.session.into_iter().map(|(pinned, _)| pinned))
            .chain(self.agent)
            .map(PinnedSource::into_fd)
    }
}

/// Der Thread, der `bwrap` startet und auf es wartet; siehe Modulbeschreibung.
fn supervise(
    program: &Path,
    args: &[OsString],
    env: &[(String, String)],
    inherit: Vec<OwnedFd>,
    stdio: StdioMode,
    shared: &Arc<Shared>,
    tx: &Sender<Result<u32, Diagnostic>>,
) {
    // Das Pseudoterminal zuerst: Ein Fehler dabei endet, bevor ein Kommando
    // gebaut ist.
    let pty = match stdio.pty_size() {
        None => None,
        Some((cols, rows)) => match open_pty(cols, rows) {
            Ok(pair) => Some(pair),
            Err(diagnostic) => {
                let _ = tx.send(Err(diagnostic));
                return;
            }
        },
    };
    let spawned = {
        let mut command = BwrapBackend::scrubbed_command(program);
        command
            .args(args)
            .envs(env.iter().map(|(key, value)| (key, value)));
        if stdio == StdioMode::Capture {
            command
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            shared.set_capturing();
        }
        if let Some(pty) = pty.as_ref() {
            // Die Untertanenseite wird für jeden der drei Deskriptoren
            // verdoppelt; `Stdio::from` verbraucht sie. Ein `dup2` im Kind
            // bräuchte einen weiteren `pre_exec`-Hook, und `unsafe` steht in
            // dieser Crate nur in `inherit.rs`.
            match pty.stdio() {
                Ok((input, output, errors)) => {
                    command.stdin(input).stdout(output).stderr(errors);
                    // Die Meldungen von `bwrap` gehen damit nicht an das
                    // geerbte stderr, sondern in dieselbe Leitung; der Befund
                    // findet sie über die Spiegelung.
                    shared.set_capturing();
                }
                Err(diagnostic) => {
                    let _ = tx.send(Err(diagnostic));
                    return;
                }
            }
        }
        // `FD_CLOEXEC` fällt nur im Kind, zwischen `fork` und `exec`; im
        // eigenen Prozess bleibt es stehen, damit kein anderer Start im
        // selben Augenblick die Deskriptoren miterbt (HUM-263).
        let borrowed: Vec<BorrowedFd<'_>> = inherit.iter().map(AsFd::as_fd).collect();
        if let Err(err) = inherit_in_child(&mut command, &borrowed) {
            let _ = tx.send(Err(handover_failed(&err)));
            return;
        }
        let spawned = command.spawn();
        // Die Kopien des Kindes bleiben; die eigenen gehen zu, damit die
        // Pipes enden, wenn die Sandbox endet.
        drop(inherit);
        spawned
    };
    let mut child = match spawned {
        Ok(child) => child,
        // Der Hook zwischen `fork` und `exec` ist gescheitert, nicht das
        // Programm: kein Installationshinweis (siehe `HOOK_ERRNO`).
        Err(err) if err.raw_os_error() == Some(HOOK_ERRNO) => {
            let _ = tx.send(Err(handover_failed(&err)));
            return;
        }
        Err(err) => {
            let _ = tx.send(Err(Diagnostic::builder(SANDBOX_001, Severity::Blocking)
                .why(format!("cannot execute {}: {err}", program.display()))
                .fix(FixAction::CopyCommand(install_command().to_owned()))
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
    if let Some(pty) = pty {
        // Die eigene Kopie der Untertanenseite geht jetzt zu; das Kind hat
        // seine. Solange sie offen bliebe, sähe die Herrscherseite nie das
        // Ende des Stroms, und wer darauf wartet, wartete für immer.
        //
        // Hier kann nichts mehr fehlschlagen, und das ist Absicht: Zwischen
        // `spawn` und der Warteschleife darf es keinen Rückweg geben, denn
        // `bwrap` läuft schon, und wer hier zurückkehrt, sammelt es nie ein.
        // Der Leser-Deskriptor ist deshalb vor dem Start verdoppelt worden
        // ([`open_pty`]).
        let Pty {
            master,
            reader,
            slave,
        } = pty;
        drop(slave);
        // Der Deskriptor steht vor der PID: Wer das Handle bekommt, hat auch
        // das Terminal, und niemand sieht ein Handle ohne eines.
        shared.set_pty(master);
        let target = Arc::clone(shared);
        shared.add_reader(thread::spawn(move || {
            drain(File::from(reader), |chunk| target.append_pty(chunk));
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
    // Weitere Shells einer Start-Tür enden mit der Sandbox (HUM-323); unter
    // Linux erledigt das der PID-Namensraum.
    shared.close_shells();
    // Erst die Leser einsammeln, dann den Zuhörer der Ausgabe loslassen, dann
    // den Status setzen. Ein Zuhörer, der bliebe, hielte seinen Kanal offen,
    // und wer auf dessen Ende wartet, wartete für immer — die Ausgabe ist zu
    // Ende, wenn die Pipes zu sind und nicht, wenn jemand das Handle fallen
    // lässt.
    shared.join_readers();
    shared.clear_sink();
    shared.set_exit(status);
}

/// Ein Pseudoterminal: die Herrscherseite bleibt beim Daemon, die
/// Untertanenseite bekommt die Sandbox als ihre drei Deskriptoren.
#[derive(Debug)]
pub(crate) struct Pty {
    pub(crate) master: OwnedFd,
    /// Dieselbe Herrscherseite ein zweites Mal, für den Leser-Faden.
    ///
    /// Sie wird **vor** dem Start verdoppelt. Danach läuft `bwrap`, und ein
    /// Fehler, der zu einem `return` führte, ließe den Prozess als Zombie
    /// stehen: Niemand riefe mehr `wait`. Ein Fehlerpfad, den es nicht gibt,
    /// ist besser als einer, den man behandelt.
    pub(crate) reader: OwnedFd,
    pub(crate) slave: File,
}

impl Pty {
    /// Dreimal dieselbe Untertanenseite: stdin, stdout, stderr.
    ///
    /// `Stdio::from` verbraucht seinen Deskriptor, also braucht jeder der drei
    /// einen eigenen. Sie zeigen auf dieselbe Beschreibung; das ist genau, was
    /// ein Terminal ist.
    pub(crate) fn stdio(&self) -> Result<(Stdio, Stdio, Stdio), Diagnostic> {
        let mut copies = Vec::with_capacity(3);
        for _ in 0..3 {
            copies.push(self.slave.try_clone().map_err(|err| {
                pty_failed(&format!(
                    "cannot hand the terminal to the sandbox as a descriptor: {err}"
                ))
            })?);
        }
        let mut copies = copies.into_iter().map(Stdio::from);
        match (copies.next(), copies.next(), copies.next()) {
            (Some(stdin), Some(stdout), Some(stderr)) => Ok((stdin, stdout, stderr)),
            // Drei wurden gebaut, drei kommen heraus; der Zweig steht nur, weil
            // `next` einen `Option` liefert und diese Crate kein `expect` außer
            // in Tests kennt.
            _ => Err(pty_failed("cannot hand the terminal to the sandbox")),
        }
    }
}

/// `O_CLOEXEC` nimmt `posix_openpt` unter macOS nicht an; dort setzt
/// [`open_pty`] das Flag danach (HUM-312).
#[cfg(target_os = "linux")]
const OPENPT_FLAGS: OpenptFlags = OpenptFlags::RDWR
    .union(OpenptFlags::NOCTTY)
    .union(OpenptFlags::CLOEXEC);
#[cfg(not(target_os = "linux"))]
const OPENPT_FLAGS: OpenptFlags = OpenptFlags::RDWR.union(OpenptFlags::NOCTTY);

/// Öffnet ein Pseudoterminal und setzt seine Geometrie.
///
/// Der Weg ist der von POSIX: `posix_openpt`, `grantpt`, `unlockpt`, den Namen
/// der Untertanenseite holen und sie öffnen. `rustix` kapselt jeden Schritt,
/// deshalb braucht keiner davon `unsafe` — das steht in dieser Crate nur in
/// `inherit.rs` (`#![deny(unsafe_code)]`).
///
/// Die Herrscherseite bekommt `CLOEXEC`: Sie darf nicht in die Sandbox
/// vererbt werden. Ein Agent mit ihr in der Hand läse mit, was ein Mensch
/// tippt, und könnte seine eigene Ausgabe fälschen.
///
/// `bwrap` ruft `setsid` selbst (`--new-session`, unbedingt in jeder
/// Kommandozeile): Der Shim beginnt ohne steuerndes Terminal. Erst sein Kind,
/// der erste Prozess der Sitzung, macht eine eigene Sitzung auf und nimmt
/// diese Untertanenseite als steuerndes Terminal (`--controlling-terminal`,
/// HUM-280); `TIOCSCTTY` ohne Übernahme gelingt nur, weil niemand sie
/// sonst als steuerndes Terminal hat, und dafür steht hier `O_NOCTTY`.
/// Damit schickt der Kernel `SIGINT` für `Ctrl+C` und `SIGWINCH` bei einer
/// Größenänderung an die Vordergrundgruppe; [`SandboxHandle::resize`] und
/// [`SandboxHandle::interrupt`] signalisieren zusätzlich selbst.
pub(crate) fn open_pty(cols: u16, rows: u16) -> Result<Pty, Diagnostic> {
    let master = openpt(OPENPT_FLAGS)
        .map_err(|err| pty_failed(&format!("cannot open a pseudo terminal: {err}")))?;
    crate::platform::set_cloexec(&master).map_err(|err| {
        pty_failed(&format!(
            "cannot mark the pseudo terminal close-on-exec: {err}"
        ))
    })?;
    grantpt(&master).map_err(|err| {
        pty_failed(&format!(
            "cannot grant the follower side of the pseudo terminal: {err}"
        ))
    })?;
    unlockpt(&master).map_err(|err| {
        pty_failed(&format!(
            "cannot unlock the follower side of the pseudo terminal: {err}"
        ))
    })?;
    let name = ptsname(&master, Vec::new()).map_err(|err| {
        pty_failed(&format!(
            "cannot name the follower side of the pseudo terminal: {err}"
        ))
    })?;
    let path = PathBuf::from(OsString::from_vec(name.into_bytes()));
    // `O_NOCTTY`: The follower side must never become the daemon's own
    // controlling terminal. A daemon under systemd is a session leader
    // without one, and an `open(2)` without the flag would make it so; the
    // session's first process could then not take it (HUM-280), and a hang-up
    // of the agent's terminal would reach the daemon.
    let slave = rustix::fs::open(
        &path,
        rustix::fs::OFlags::RDWR | rustix::fs::OFlags::NOCTTY | rustix::fs::OFlags::CLOEXEC,
        rustix::fs::Mode::empty(),
    )
    .map(File::from)
    .map_err(|err| pty_failed(&format!("cannot open {}: {err}", path.display())))?;
    tcsetwinsize(
        &master,
        Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        },
    )
    .map_err(|err| pty_failed(&format!("cannot size the pseudo terminal: {err}")))?;
    let reader = master.try_clone().map_err(|err| {
        pty_failed(&format!(
            "cannot duplicate the terminal of the sandbox: {err}"
        ))
    })?;
    Ok(Pty {
        master,
        reader,
        slave,
    })
}

/// Der Befund, wenn sich kein Pseudoterminal aufbauen lässt.
fn pty_failed(why: &str) -> Diagnostic {
    Diagnostic::builder(TERM_002, Severity::Blocking)
        .why(why.to_owned())
        .build()
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
        if let Some(check) = parse_check_line(&line) {
            shared.push_check(check);
        } else if let Some(failure) = parse_exec_line(&line) {
            shared.set_exec_failed(failure);
        } else if let Some(refusal) = crate::refusals::parse_refusal_line(&line) {
            shared.push_refusal(refusal);
        } else {
            shared.push_other_line();
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

/// Die Deskriptoren ließen sich `bwrap` nicht übergeben (HUM-263).
fn handover_failed(err: &std::io::Error) -> Diagnostic {
    Diagnostic::builder(SANDBOX_012, Severity::Blocking)
        .why(format!(
            "cannot hand the inherited descriptors to bwrap: {err}; the sandbox was not started"
        ))
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

    #[cfg(target_os = "linux")]
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
        crate::test_support::write_program(&fake, "#!/bin/sh\necho bubblewrap 0.0.1\n");
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
    // `memfd_create` gibt es nur unter Linux (HUM-312).
    #[cfg(target_os = "linux")]
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
