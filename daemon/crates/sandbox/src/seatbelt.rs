//! Das Sandbox-Backend für macOS: Seatbelt (HUM-323, `backlog/sprint-6.md`).
//!
//! Seatbelt ist die Sandbox des macOS-Kerns. `sandbox-exec -f profil.sb`
//! setzt ein Profil in der Sprache SBPL, und der Kern hält den Prozess und
//! jedes seiner Kinder daran; Chrome, `Codex` von `OpenAI` und `Claude Code`
//! (`sandbox-runtime`) nutzen es so. Ein Kind kann das Profil nicht ablegen.
//!
//! **Was anders ist als unter Linux.** Es gibt keine Namensräume und keine
//! Mounts. Der Agent sieht echte Pfade, und das Profil sagt je Pfad, was er
//! darf. Deshalb legt der Plan je Sitzung ein privates Verzeichnis an
//! ([`SessionLayout`]) und bildet die Sandbox-Pfade des Profils darauf ab
//! ([`PathMap`]): `/work` ist der Projektordner selbst, `/home/agent`,
//! `/tmp`, `/etc/humanitl` und `/run/humanitl` liegen im
//! Sitzungsverzeichnis. Die Abbildung gilt für die Umgebung, das Kommando
//! und den Text der Adapter-Dateien, damit ein Verweis wie
//! `/etc/humanitl/pi/llm-api-key` dort ankommt, wo die Datei liegt.
//!
//! **Das Profil** ([`SeatbeltPolicy::render`]) verbietet alles und erlaubt
//! dann: Lesen überall außer unter `/Users`, `/Volumes` und den temporären
//! Verzeichnissen; dort nur das Projekt, das Sitzungsverzeichnis, die
//! zusätzlichen Ordner des Profils und einzelne Dateien (Shim, Agent).
//! Schreiben nur im Projekt (bei `rw`), im Sitzungsverzeichnis und an den
//! Terminals. Netz nur zum Proxy-Socket und zum Loopback-Port der Sitzung.
//! Die Masken des Profils werden zu Verboten auf genau diesen Pfaden. In SBPL
//! gewinnt die letzte passende Regel, deshalb stehen die Verbote am Ende.
//!
//! **Die Tür.** Auch hier lauscht der Shim auf Loopback und reicht an den
//! Proxy-Socket weiter. Loopback ist unter macOS mit dem Host geteilt, also
//! bekommt jede Sitzung einen freien Port, und das Profil erlaubt nur ihn.
//!
//! **Die Garantien** misst der Shim von innen und meldet sie mit denselben
//! fünf `CHECK`-Zeilen wie unter Linux; die Zuordnung zu den drei Garantien
//! ist dieselbe ([`crate::bwrap`]). Was sie unter macOS bedeuten, steht in
//! `bin/humanitl-shim/src/macos.rs` und in `docs/SECURITY.md`.

use humanitl_core::shell::shell_path;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpListener};
use std::os::fd::{AsRawFd as _, OwnedFd};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use humanitl_config::Paths;
use humanitl_config::WorkMode;
use humanitl_core::diagnostics::codes::{SANDBOX_001, SANDBOX_011};
use humanitl_core::{Diagnostic, Severity};

use crate::bridge_env::bridges_json;
use crate::bwrap::{BwrapBackend, REPORT_TIMEOUT};
use crate::handle::{OutputSink, SandboxHandle};
use crate::launcher::{CheckResult, LaunchOnce, LaunchPlan, SandboxBackend, StdioMode};
use crate::profile::{
    Bridge, BridgeDirection, CA_BUNDLE_DST, CA_CERT_DST, MountPolicy, PROXY_SOCKET_DST, SHIM_DST,
    SandboxProfile, SessionContext, WORK_DST,
};

/// Das Programm, das ein Profil setzt und dann das Kommando startet.
pub const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// Der Name des Profils im Sitzungsverzeichnis.
pub const PROFILE_FILE: &str = "sandbox.sb";

/// Wie alt ein Sitzungsverzeichnis werden darf, bevor ein neuer Plan es
/// wegräumt. Eine Sitzung, die so lange läuft, behält ihr Verzeichnis, weil
/// der Shim es bis zum Ende offen hält und es dabei jünger bleibt
/// (`home` und `tmp` ändern sich mit jeder Arbeit).
pub const STALE_SESSION: Duration = Duration::from_secs(7 * 24 * 3600);

/// Die Suchpfade unter macOS, nach dem Verzeichnis des Agenten.
pub const SEARCH_PATH: &str = "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin";

/// Wo unter macOS niemand außer dem Profil lesen darf: Heimverzeichnisse,
/// Laufwerke und die temporären Verzeichnisse aller Programme.
pub const PRIVATE_TREES: &[&str] = &["/Users", "/Volumes", "/private/var/folders", "/private/tmp"];

/// Die Mach-Dienste, die gewöhnliche Werkzeuge brauchen und die nichts nach
/// außen tragen: Benutzerdaten (`getpwuid`), Benachrichtigungen, Protokoll,
/// Einstellungen. Nicht dabei: `mDNSResponder` (DNS), `trustd` (lädt
/// Zertifikate nach), der Pasteboard-Server, `launchd` selbst und jeder Dienst
/// des Schlüsselbunds (`securityd`): `/usr/bin/security` darf der Agent
/// starten, doch ohne diesen Dienst findet es nichts (HUM-349).
pub const MACH_SERVICES: &[&str] = &[
    "com.apple.system.opendirectoryd.libinfo",
    "com.apple.system.notification_center",
    "com.apple.system.logger",
    "com.apple.logd",
    "com.apple.cfprefsd.daemon",
    "com.apple.cfprefsd.agent",
];

/// Die Abbildung der Sandbox-Pfade des Profils auf echte Pfade.
///
/// Geordnet vom längsten Präfix zum kürzesten, damit
/// `/run/humanitl/proxy.sock` vor `/run/humanitl` greift.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathMap {
    entries: Vec<(String, PathBuf)>,
}

impl PathMap {
    /// Eine Abbildung aus Paaren `(Sandbox-Pfad, Host-Pfad)`.
    #[must_use]
    pub fn new(entries: impl IntoIterator<Item = (String, PathBuf)>) -> Self {
        let mut entries: Vec<(String, PathBuf)> = entries
            .into_iter()
            .map(|(from, to)| (from.trim_end_matches('/').to_owned(), to))
            .collect();
        entries.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
        Self { entries }
    }

    /// Der Host-Pfad für einen Sandbox-Pfad, oder `None`, wenn kein Eintrag
    /// passt. Ein Eintrag passt auf sich selbst und auf alles darunter.
    #[must_use]
    pub fn map_path(&self, path: &Path) -> Option<PathBuf> {
        let text = path.to_str()?;
        self.entries.iter().find_map(|(from, to)| {
            let rest = text.strip_prefix(from.as_str())?;
            if rest.is_empty() {
                return Some(to.clone());
            }
            // Nur Bestandteile unterhalb des Ziels: `//x` oder `..` führten
            // mit `join` hinaus (Review HUM-323).
            let rest = Path::new(rest.strip_prefix('/')?);
            rest.components()
                .all(|component| matches!(component, Component::Normal(_)))
                .then(|| to.join(rest))
        })
    }

    /// Ersetzt jeden Sandbox-Pfad in einem Text durch seinen Host-Pfad, aber
    /// nur an Pfadgrenzen: `/work` in `/workspace` oder in `a/work` bleibt.
    #[must_use]
    pub fn map_text(&self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut at = 0;
        let bytes = text.as_bytes();
        'scan: while at < text.len() {
            let before_ok = at == 0 || !is_path_byte(bytes[at - 1]);
            if before_ok && bytes[at] == b'/' {
                for (from, to) in &self.entries {
                    let end = at + from.len();
                    if text[at..].starts_with(from.as_str())
                        && bytes
                            .get(end)
                            .is_none_or(|&next| next == b'/' || !is_path_byte(next))
                    {
                        out.push_str(&to.to_string_lossy());
                        at = end;
                        continue 'scan;
                    }
                }
            }
            let ch = text[at..].chars().next().unwrap_or('\0');
            out.push(ch);
            at += ch.len_utf8().max(1);
        }
        out
    }
}

/// Ob ein Byte zu einem Pfadnamen gehören kann, für die Grenzen in
/// [`PathMap::map_text`].
const fn is_path_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'/' | b'~')
}

/// Das private Verzeichnis einer Sitzung und seine Teile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionLayout {
    /// Das Verzeichnis selbst, 0700.
    pub root: PathBuf,
}

impl SessionLayout {
    /// Das Heimverzeichnis des Agenten.
    #[must_use]
    pub fn home(&self) -> PathBuf {
        self.root.join("home")
    }

    /// Das temporäre Verzeichnis (`/tmp`, `/var/tmp`, `TMPDIR`).
    #[must_use]
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    /// Das Gegenstück zu `/etc` für die Dateien von Humanitl.
    #[must_use]
    pub fn etc(&self) -> PathBuf {
        self.root.join("etc")
    }

    /// Das Git-Verzeichnis, das der Agent statt `.git/config` sieht (HUM-366).
    #[must_use]
    pub fn git(&self) -> PathBuf {
        self.root.join("git")
    }

    /// Wo das `git` der Sitzung liegt (HUM-366), außerhalb des Schreibbaren.
    #[must_use]
    pub fn git_bin(&self) -> PathBuf {
        self.root.join("bin")
    }

    /// Das Gegenstück zu `/run/humanitl`.
    #[must_use]
    pub fn run(&self) -> PathBuf {
        self.root.join("run")
    }

    /// Das Profil.
    #[must_use]
    pub fn profile(&self) -> PathBuf {
        self.root.join(PROFILE_FILE)
    }

    /// Die Abbildung der Sandbox-Pfade für diese Sitzung.
    #[must_use]
    pub fn path_map(&self, work: &Path, door: &Path, shim: &Path) -> PathMap {
        PathMap::new([
            (WORK_DST.to_owned(), work.to_path_buf()),
            (crate::DEFAULT_HOME.to_owned(), self.home()),
            ("/tmp".to_owned(), self.tmp()),
            ("/var/tmp".to_owned(), self.tmp()),
            (
                crate::agent::AGENT_CONFIG_DIR.to_owned(),
                self.etc().join("humanitl"),
            ),
            (
                CA_BUNDLE_DST.to_owned(),
                self.etc().join("ssl/certs/ca-certificates.crt"),
            ),
            (PROXY_SOCKET_DST.to_owned(), door.to_path_buf()),
            (SHIM_DST.to_owned(), shim.to_path_buf()),
            ("/run/humanitl".to_owned(), self.run()),
        ])
    }
}

/// Was das Profil einer Sitzung erlaubt; [`SeatbeltPolicy::render`] macht
/// daraus SBPL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeatbeltPolicy {
    /// Der Projektordner, aufgelöst.
    pub work: PathBuf,
    /// Ob der Agent im Projekt schreiben darf.
    pub work_writable: bool,
    /// Das Sitzungsverzeichnis, nur lesbar: Darin liegen das Profil selbst,
    /// die Dateien des Adapters und der Verweis auf den Agenten.
    pub session: PathBuf,
    /// Die beschreibbaren Teile der Sitzung, `home` und `tmp`. Nie das
    /// Sitzungsverzeichnis selbst: Wer dort schreiben dürfte, ersetzte das
    /// Profil, mit dem die nächste Shell startet (Review HUM-323).
    pub session_writable: Vec<PathBuf>,
    /// Weitere Ordner, nur lesbar.
    pub read_only: Vec<PathBuf>,
    /// Weitere Ordner, lesbar und beschreibbar.
    pub writable: Vec<PathBuf>,
    /// Einzelne Dateien, nur lesbar (Shim, Agent-Programm).
    pub files: Vec<PathBuf>,
    /// Pfade unter dem Projekt, die weder gelesen noch geschrieben werden
    /// (`masked_files` und die `tmpfs` unter `/work`).
    pub masked: Vec<PathBuf>,
    /// Ordner zwischen Projekt und einer Maske; sie dürfen weder umbenannt
    /// noch gelöscht werden.
    pub locked: Vec<PathBuf>,
    /// Der Proxy-Socket, die einzige Tür.
    pub door: PathBuf,
    /// Der Loopback-Port der Sitzung.
    pub port: u16,
    /// Der zweite Port der Sitzung: Hier darf ein Agent seinen eigenen lokalen
    /// Server betreiben (HUM-365). Unter Linux hat er dafür ein ganzes eigenes
    /// Loopback; hier ist Loopback geteilt, und das Profil nennt deshalb genau
    /// einen zufälligen Port statt eines festen, hinter dem ein Dienst des
    /// Hosts lauschen könnte.
    pub agent_port: u16,
}

impl SeatbeltPolicy {
    /// Der Netzwerkteil des Profils.
    fn render_network(&self, out: &mut String) {
        out.push_str(
            ";; Network: the proxy socket and the session's two loopback ports, nothing else.\n",
        );
        let _ = writeln!(
            out,
            "(allow network-outbound (remote unix-socket (path-literal {})))",
            quote_path(&self.door)
        );
        for port in [self.port, self.agent_port] {
            let local = quote(&format!("localhost:{port}"));
            let _ = writeln!(out, "(allow network-outbound (remote ip {local}))");
            let _ = writeln!(out, "(allow network-bind (local ip {local}))");
            let _ = writeln!(out, "(allow network-inbound (local ip {local}))");
        }
        out.push('\n');
    }

    /// Das Profil als SBPL-Text.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(
            ";; Humanitl Seatbelt profile (HUM-323). Generated per session; do not edit.\n\
             ;; In SBPL the last matching rule wins.\n\
             (version 1)\n(deny default)\n\n\
             ;; Processes: start programs, only signal or inspect the own sandbox.\n\
             (allow process-exec)\n(allow process-fork)\n\
             (allow signal (target same-sandbox))\n\
             (allow process-info* (target same-sandbox))\n\
             (allow sysctl-read)\n\
             (allow pseudo-tty)\n\
             (allow file-ioctl (literal \"/dev/tty\") (regex #\"^/dev/ttys[0-9]+$\"))\n\
             (allow ipc-posix-sem)\n(allow ipc-posix-shm)\n",
        );
        out.push_str("(allow mach-lookup");
        for service in MACH_SERVICES {
            let _ = write!(out, " (global-name {})", quote(service));
        }
        out.push_str(")\n\n");

        out.push_str(
            ";; Reading: everything but the private trees, then what the session needs.\n",
        );
        out.push_str("(allow file-read*)\n(deny file-read*");
        for tree in PRIVATE_TREES {
            let _ = write!(out, " (subpath {})", quote(tree));
        }
        out.push_str(")\n");
        let mut readable = vec![&self.work, &self.session];
        readable.extend(&self.read_only);
        readable.extend(&self.writable);
        out.push_str("(allow file-read*");
        for dir in &readable {
            let _ = write!(out, " (subpath {})", quote_path(dir));
        }
        for file in self.files.iter().chain(std::iter::once(&self.door)) {
            let _ = write!(out, " (literal {})", quote_path(file));
        }
        out.push_str(")\n");
        // Wer `/Users/name/projekt` öffnet, muss die Ordner darüber nennen
        // können (`realpath`, `getcwd`), aber nicht ihren Inhalt lesen.
        let mut ancestors: Vec<&Path> = readable
            .iter()
            .map(|dir| dir.as_path())
            .chain(self.files.iter().map(PathBuf::as_path))
            .chain(std::iter::once(self.door.as_path()))
            .flat_map(Path::ancestors)
            .collect();
        ancestors.sort();
        ancestors.dedup();
        out.push_str("(allow file-read-metadata");
        for dir in ancestors {
            let _ = write!(out, " (literal {})", quote_path(dir));
        }
        out.push_str(")\n\n");

        out.push_str(";; Writing: the session, the project when it is writable, the terminals.\n");
        out.push_str("(allow file-write*");
        for dir in &self.session_writable {
            let _ = write!(out, " (subpath {})", quote_path(dir));
        }
        if self.work_writable {
            let _ = write!(out, " (subpath {})", quote_path(&self.work));
        }
        for dir in &self.writable {
            let _ = write!(out, " (subpath {})", quote_path(dir));
        }
        // `/dev/stdout` und `/dev/fd/N` öffnen nur, was der Prozess schon hält.
        out.push_str(
            " (literal \"/dev/null\") (literal \"/dev/tty\") (literal \"/dev/stdout\") \
             (literal \"/dev/stderr\") (regex #\"^/dev/fd/[0-9]+$\"))\n",
        );
        // Fremde Terminals des Nutzers bleiben zu: Das eigene erbt der Agent
        // als Deskriptor und erreicht es als `/dev/tty`; `stat` bleibt
        // erlaubt, sonst fände `ttyname(3)` das eigene nicht. Ein neues legt er
        // über `/dev/ptmx` an; dessen Untertanenseite trägt dann die
        // Erweiterung `com.apple.sandbox.pty`, die nur er bekommt (Review
        // HUM-323).
        out.push_str(
            "(deny file-read-data file-write* (regex #\"^/dev/ttys[0-9]+$\"))\n\
             (allow file-read* file-write* file-ioctl (literal \"/dev/ptmx\"))\n\
             (allow file-read-data file-write* (require-all (regex #\"^/dev/ttys[0-9]+$\") \
             (extension \"com.apple.sandbox.pty\")))\n\n",
        );

        self.render_network(&mut out);

        if !self.masked.is_empty() {
            out.push_str(";; Masks of the profile: neither read nor written.\n");
            out.push_str("(deny file-read* file-write*");
            for path in &self.masked {
                let _ = write!(out, " (subpath {})", quote_path(path));
            }
            out.push_str(")\n");
        }
        if !self.locked.is_empty() {
            out.push_str("(deny file-write*");
            for path in &self.locked {
                let _ = write!(out, " (literal {})", quote_path(path));
            }
            out.push_str(")\n");
        }
        out
    }
}

/// Ein SBPL-Zeichenkettenliteral.
fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        if matches!(ch, '"' | '\\') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.push('"');
    out
}

fn quote_path(path: &Path) -> String {
    quote(&path.to_string_lossy())
}

/// Das Seatbelt-Backend.
#[derive(Debug, Clone)]
pub struct SeatbeltBackend {
    program: PathBuf,
    paths: Paths,
    stdio: StdioMode,
    report_timeout: Duration,
    output_sink: Option<OutputSink>,
    read_only_binds: Vec<(PathBuf, PathBuf)>,
    approved_work_dir: Option<PathBuf>,
}

impl SeatbeltBackend {
    /// Findet `sandbox-exec`.
    ///
    /// # Errors
    ///
    /// `SANDBOX_001`, wenn es fehlt; es gehört zu jedem macOS.
    pub fn detect(paths: Paths) -> Result<Self, Diagnostic> {
        let program = PathBuf::from(SANDBOX_EXEC);
        if !program.is_file() {
            return Err(Diagnostic::builder(SANDBOX_001, Severity::Blocking)
                .why(format!(
                    "{SANDBOX_EXEC} is missing; it is part of macOS and the sandbox needs it"
                ))
                .build());
        }
        Ok(Self::unchecked(program, paths))
    }

    /// Ohne Prüfung, für Tests und den Rückfall, wenn `detect` scheitert.
    #[must_use]
    pub fn unchecked(program: impl Into<PathBuf>, paths: Paths) -> Self {
        Self {
            program: program.into(),
            paths,
            stdio: StdioMode::default(),
            report_timeout: REPORT_TIMEOUT,
            output_sink: None,
            read_only_binds: Vec::new(),
            approved_work_dir: None,
        }
    }

    /// Ohne Prüfung, mit [`SANDBOX_EXEC`]: der Rückfall, wenn [`Self::detect`]
    /// scheitert (dieselbe Form wie [`BwrapBackend::fallback`]).
    #[must_use]
    pub fn fallback(paths: Paths) -> Self {
        Self::unchecked(SANDBOX_EXEC, paths)
    }

    /// Wohin Ein- und Ausgabe gehen.
    #[must_use]
    pub const fn with_stdio(mut self, stdio: StdioMode) -> Self {
        self.stdio = stdio;
        self
    }

    /// Wer die Ausgabe mitliest.
    #[must_use]
    pub fn with_output_sink(mut self, sink: OutputSink) -> Self {
        self.output_sink = Some(sink);
        self
    }

    /// Ein weiterer nur lesbarer Ort. Unter macOS liegt er unter seinem
    /// eigenen Pfad; das Ziel unter `/run/humanitl` wird darauf abgebildet.
    #[must_use]
    pub fn with_read_only_bind(mut self, src: impl Into<PathBuf>, dst: impl Into<PathBuf>) -> Self {
        self.read_only_binds.push((src.into(), dst.into()));
        self
    }

    /// Der Projektordner, wie die Prüfung ihn aufgelöst hat.
    #[must_use]
    pub fn with_approved_work_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.approved_work_dir = Some(dir.into());
        self
    }

    /// Der geprüfte Projektordner.
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

    /// `sandbox-exec`.
    #[must_use]
    pub fn program(&self) -> &Path {
        &self.program
    }

    /// Wo die Sitzungsverzeichnisse liegen.
    fn sessions_dir(&self) -> PathBuf {
        self.paths.runtime_dir().path.join("sessions")
    }

    /// Weitere Shells: derselbe Shim ohne Bridge unter demselben Profil, nur am
    /// eigenen Terminal des Daemons (wie unter Linux, HUM-299). Den Bericht und
    /// die Bridge der Sitzung bekommen sie nicht.
    fn shell_seed(
        &self,
        layout: &SessionLayout,
        shim: &Path,
        work: &Path,
        env: &[(String, String)],
    ) -> Option<crate::shell::DoorSeed> {
        self.stdio.pty_size().map(|_| {
            crate::shell::DoorSeed::Spawn(crate::shell::SpawnSpec {
                program: self.program.clone(),
                args: vec![
                    "-f".into(),
                    layout.profile().into_os_string(),
                    shim.as_os_str().to_owned(),
                    "--no-bridge".into(),
                    "--chdir".into(),
                    work.as_os_str().to_owned(),
                    "--controlling-terminal".into(),
                ],
                env: env
                    .iter()
                    .filter(|(key, _)| {
                        key != crate::bridge_env::ENV_REPORT_FD
                            && key != crate::bridge_env::ENV_BRIDGES
                    })
                    .cloned()
                    .collect(),
            })
        })
    }

    /// Legt das Verzeichnis der Sitzung frisch an und räumt alte weg.
    fn create_layout(&self, session: &SessionContext) -> Result<SessionLayout, Diagnostic> {
        let parent = self.sessions_dir();
        std::fs::create_dir_all(&parent)
            .and_then(|()| {
                std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o700))
            })
            .map_err(|err| layout_failed(&parent, &err))?;
        sweep_stale(&parent);
        let root = parent.join(session.session.to_string());
        if root.exists() {
            std::fs::remove_dir_all(&root).map_err(|err| layout_failed(&root, &err))?;
        }
        let layout = SessionLayout { root };
        for dir in [
            layout.root.clone(),
            layout.home(),
            layout.tmp(),
            layout.etc().join("humanitl"),
            layout.etc().join("ssl/certs"),
            layout.run().join("agent"),
        ] {
            std::fs::create_dir_all(&dir)
                .and_then(|()| {
                    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
                })
                .map_err(|err| layout_failed(&dir, &err))?;
        }
        // Kanonisch: Das Profil vergleicht aufgelöste Pfade, und unter macOS
        // liegt das temporäre Verzeichnis hinter dem Verweis `/var`.
        let root = layout
            .root
            .canonicalize()
            .map_err(|err| layout_failed(&layout.root, &err))?;
        Ok(SessionLayout { root })
    }

    /// Was sich vor dem Start prüfen lässt; der aufgelöste Projektordner.
    fn check(
        &self,
        profile: &SandboxProfile,
        session: &SessionContext,
    ) -> Result<PathBuf, Diagnostic> {
        let policy = MountPolicy::from_paths(&self.paths);
        profile.validate_with(&policy)?;
        BwrapBackend::check_work_dir(profile, session)?;
        policy.check_work_dir(&session.work_src)?;
        let work = session.work_src.canonicalize().map_err(|err| {
            Diagnostic::builder(
                humanitl_core::diagnostics::codes::SANDBOX_006,
                Severity::Blocking,
            )
            .why(format!(
                "the project directory {} cannot be resolved: {err}",
                session.work_src.display()
            ))
            .build()
        })?;
        policy.check_work_dir(&work)?;
        if let Some(approved) = &self.approved_work_dir
            && work != *approved
        {
            return Err(Diagnostic::builder(
                humanitl_core::diagnostics::codes::SANDBOX_006,
                Severity::Blocking,
            )
            .why(format!(
                "the project directory {} was checked at {}, but it lies at {} now; \
                 a symbolic link in its path leads elsewhere, so it is not used",
                session.work_src.display(),
                approved.display(),
                work.display()
            ))
            .fix(crate::work_dir::choose_folder_fix())
            .build());
        }
        BwrapBackend::check_proxy_socket_in(&self.paths, session)?;
        BwrapBackend::check_regular_file(&session.ca_cert_src, "CA certificate")?;
        BwrapBackend::check_regular_file(&session.ca_bundle_src, "CA bundle")?;
        BwrapBackend::check_shim(session)?;
        Ok(work)
    }

    /// Das Profil der Sitzung.
    #[allow(clippy::too_many_arguments)]
    fn policy(
        &self,
        profile: &SandboxProfile,
        session: &SessionContext,
        work: &Path,
        layout: &SessionLayout,
        files: Vec<PathBuf>,
        door: &Path,
        ports: (u16, u16),
    ) -> SeatbeltPolicy {
        // Masken wie unter Linux nur über das, was es im Projekt gibt; was
        // fehlt, nennt der Plan als ungeschützt (HUM-043).
        let present = BwrapBackend::present_under_work(profile, work);
        let masked: Vec<PathBuf> = present
            .iter()
            .filter_map(|path| under_work(path))
            .filter(|rest| !rest.as_os_str().is_empty())
            .map(|rest| work.join(rest))
            .collect();
        // Die Ordner zwischen Projekt und Maske dürfen weder umbenannt noch
        // gelöscht werden: Sonst zöge `mv .git .gitx` die Maske von
        // `.git/hooks` ab, und `mv .gitx .git` brächte den Inhalt zurück
        // (Review HUM-323).
        let mut locked: Vec<PathBuf> = masked
            .iter()
            .flat_map(|path| path.ancestors().skip(1))
            .filter(|dir| dir.starts_with(work) && *dir != work)
            .map(Path::to_path_buf)
            .collect();
        locked.sort();
        locked.dedup();

        let mut read_only: Vec<PathBuf> = profile
            .mounts
            .extra_ro
            .iter()
            .filter_map(|path| path.canonicalize().ok())
            .collect();
        read_only.extend(
            self.read_only_binds
                .iter()
                .filter_map(|(src, _)| src.canonicalize().ok()),
        );
        let writable: Vec<PathBuf> = profile
            .mounts
            .extra_rw
            .iter()
            .filter_map(|path| path.canonicalize().ok())
            .collect();
        SeatbeltPolicy {
            work: work.to_path_buf(),
            work_writable: profile.effective_work_mode(session.work_mode) == WorkMode::Rw,
            session: layout.root.clone(),
            session_writable: vec![layout.home(), layout.tmp(), layout.git()],
            read_only,
            writable,
            files,
            masked,
            locked,
            door: door.to_path_buf(),
            port: ports.0,
            agent_port: ports.1,
        }
    }
}

/// Entfernt Sitzungsverzeichnisse, die älter sind als [`STALE_SESSION`].
fn sweep_stale(parent: &Path) {
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let old = entry
            .metadata()
            .and_then(|meta| meta.modified())
            .ok()
            .and_then(|at| SystemTime::now().duration_since(at).ok())
            .is_some_and(|age| age > STALE_SESSION);
        if old {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

fn layout_failed(path: &Path, err: &std::io::Error) -> Diagnostic {
    Diagnostic::builder(SANDBOX_011, Severity::Blocking)
        .why(format!(
            "cannot prepare the session directory {}: {err}",
            path.display()
        ))
        .build()
}

/// Der aufgelöste Pfad einer Datei, die es geben muss.
fn canonical(path: &Path) -> Result<PathBuf, Diagnostic> {
    path.canonicalize().map_err(|err| layout_failed(path, &err))
}

/// Ein freier Loopback-Port für die Tür dieser Sitzung.
/// Die Ports, die `OpenCode` für seinen Hintergrunddienst fest wählt (HUM-365).
/// Hinter ihnen kann ein Dienst des Hosts lauschen, also sind sie nie der Port
/// einer Sitzung.
const AGENT_SERVICE_PORTS: [u16; 2] = [49_374, 49_375];

/// Ein Port für diese Sitzung: frei auf beiden Loopback-Adressen, denn das
/// Profil kennt nur `localhost` und erlaubt damit IPv4 und IPv6 zugleich. Wer
/// auf `[::1]` lauscht, stünde sonst dem Agenten offen (HUM-365).
fn session_port(taken: Option<u16>) -> Result<u16, Diagnostic> {
    loop {
        let port = free_port()?;
        if Some(port) == taken || AGENT_SERVICE_PORTS.contains(&port) {
            continue;
        }
        let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, port));
        if v6.is_ok() || v6.is_err_and(|err| err.kind() == std::io::ErrorKind::AddrNotAvailable) {
            return Ok(port);
        }
    }
}

fn free_port() -> Result<u16, Diagnostic> {
    TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0)))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .map_err(|err| {
            Diagnostic::builder(SANDBOX_011, Severity::Blocking)
                .why(format!("no free loopback port for the proxy bridge: {err}"))
                .build()
        })
}

/// Schreibt eine Datei neu, mit genau diesem Modus.
fn write_file(path: &Path, content: &[u8], mode: u32) -> Result<(), Diagnostic> {
    use std::io::Write as _;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| layout_failed(parent, &err))?;
    }
    let _ = std::fs::remove_file(path);
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .and_then(|mut file| file.write_all(content))
        .and_then(|()| std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)))
        .map_err(|err| layout_failed(path, &err))
}

/// Legt CA, Bündel und die Dateien des Adapters ins Sitzungsverzeichnis,
/// ihren Text auf die echten Pfade abgebildet.
fn place_files(
    session: &SessionContext,
    map: &PathMap,
    warnings: &mut Vec<Diagnostic>,
) -> Result<(), Diagnostic> {
    // Die Dateien der Sitzung: CA, Bündel, Adapter-Dateien.
    let ca = std::fs::read(&session.ca_cert_src)
        .map_err(|err| layout_failed(&session.ca_cert_src, &err))?;
    let bundle = std::fs::read(&session.ca_bundle_src)
        .map_err(|err| layout_failed(&session.ca_bundle_src, &err))?;
    let mapped = |dst: &str| map.map_path(Path::new(dst)).unwrap_or_default();
    write_file(&mapped(CA_CERT_DST), &ca, 0o444)?;
    write_file(&mapped(CA_BUNDLE_DST), &bundle, 0o444)?;
    for file in &session.files {
        let Some(target) = map
            .map_path(&file.dst)
            .filter(|_| under_work(&file.dst).is_none())
        else {
            warnings.push(
                Diagnostic::builder(SANDBOX_011, Severity::Warning)
                    .why(format!(
                        "{} has no place in the macOS sandbox and is left out",
                        file.dst.display()
                    ))
                    .build(),
            );
            continue;
        };
        let content = if file.secret {
            file.content.clone()
        } else {
            match std::str::from_utf8(&file.content) {
                Ok(text) => map.map_text(text).into_bytes(),
                Err(_) => file.content.clone(),
            }
        };
        write_file(&target, &content, file.mode)?;
    }
    Ok(())
}

/// Was aus der echten `.git/config` in die der Sitzung wandert (HUM-366):
/// Schlüssel und erlaubte Werte. Ohne das Format läse Git ein sha256- oder
/// reftable-Repository falsch und beschädigte dessen Index; mehr braucht es
/// nicht, und Zugangsdaten oder `core.sshCommand` kommen so nie durch.
fn allowed_git_value(section: &str, key: &str, value: &str) -> bool {
    let boolean = matches!(value, "true" | "false");
    match (section, key) {
        ("core", "repositoryformatversion") => matches!(value, "0" | "1"),
        ("core", "ignorecase" | "precomposeunicode") => boolean,
        ("extensions", "objectformat") => matches!(value, "sha1" | "sha256"),
        ("extensions", "refstorage") => matches!(value, "files" | "reftable"),
        _ => false,
    }
}

/// Die `config` der Sitzung aus der echten: nur, was
/// [`allowed_git_value`] durchlässt.
fn git_format_config(real: &str) -> String {
    let mut sections: std::collections::BTreeMap<String, Vec<String>> =
        std::collections::BTreeMap::new();
    let mut section = String::new();
    for line in real.lines().map(str::trim) {
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            name.trim().to_ascii_lowercase().clone_into(&mut section);
        } else if let Some((key, value)) = line.split_once('=') {
            let (key, value) = (key.trim().to_ascii_lowercase(), value.trim());
            if allowed_git_value(&section, &key, value) {
                sections
                    .entry(section.clone())
                    .or_default()
                    .push(format!("\t{key} = {value}\n"));
            }
        }
    }
    let mut out = String::new();
    for (name, lines) in sections {
        let _ = writeln!(out, "[{name}]");
        out.extend(lines);
    }
    out
}

/// Das `git`, das der Agent zuerst auf dem Suchpfad findet. Es setzt die
/// Variablen nur, wenn das Repository an diesem Ort das Projekt ist; in jedem
/// anderen Repository, bei `clone` und `init` ist es das echte Git.
fn git_wrapper(real: &Path, work: &Path, shadow: &Path) -> String {
    let real = shell_path(real);
    let git_dir = shell_path(&work.join(".git"));
    let tree = shell_path(work);
    let common = shell_path(shadow);
    format!(
        r#"#!/bin/sh
dir=$(pwd -P)
skip=
for arg in "$@"; do
  if [ -n "$skip" ]; then
    [ "$skip" = C ] && dir=$(cd "$dir" 2>/dev/null && cd "$arg" 2>/dev/null && pwd -P)
    skip=
    continue
  fi
  case "$arg" in
    -C) skip=C ;;
    -c) skip=c ;;
    clone|init) exec {real} "$@" ;;
    -*) ;;
    *) break ;;
  esac
done
while [ -n "$dir" ]; do
  if [ -e "$dir/.git" ]; then
    if [ "$dir" = {tree} ]; then
      GIT_DIR={git_dir} GIT_COMMON_DIR={common} GIT_WORK_TREE={tree}
      export GIT_DIR GIT_COMMON_DIR GIT_WORK_TREE
    fi
    break
  fi
  dir=${{dir%/*}}
done
exec {real} "$@"
"#
    )
}

/// Macht Git im Projekt benutzbar, obwohl `.git/config` maskiert ist (HUM-366).
///
/// Unter Linux liegt über der Datei eine leere; Seatbelt kann nur verbieten,
/// und Git bricht ab, wenn es sie nicht lesen kann. `GIT_COMMON_DIR` zeigt
/// deshalb auf ein Verzeichnis der Sitzung mit gefilterter `config` und leeren
/// Hooks, dessen Objekte und Refs die echten sind. `HEAD` und `index` bleiben
/// über `GIT_DIR` die des Projekts. Die Maske selbst ändert das nicht. Gesetzt
/// werden die Variablen von [`git_wrapper`], nicht in der Umgebung: Dort
/// kaperten sie jedes andere Repository. Gibt das Verzeichnis des Wrappers
/// zurück; ohne `.git`-Verzeichnis, ohne Maske oder ohne Git gibt es nichts zu tun.
fn place_git(
    layout: &SessionLayout,
    work: &Path,
    masked: &[PathBuf],
) -> Result<Option<PathBuf>, Diagnostic> {
    let real = work.join(".git");
    if !real.is_dir() || !masked.contains(&real.join("config")) {
        return Ok(None);
    }
    let Some(program) = SEARCH_PATH
        .split(':')
        .map(|dir| Path::new(dir).join("git"))
        .find(|path| path.is_file())
    else {
        return Ok(None);
    };
    let shadow = layout.git();
    std::fs::create_dir_all(shadow.join("hooks")).map_err(|err| layout_failed(&shadow, &err))?;
    let config = std::fs::read_to_string(real.join("config")).unwrap_or_default();
    write_file(
        &shadow.join("config"),
        git_format_config(&config).as_bytes(),
        0o600,
    )?;
    for entry in [
        "objects",
        "refs",
        "packed-refs",
        "logs",
        "info",
        "shallow",
        "reftable",
        "worktrees",
        "modules",
    ] {
        std::os::unix::fs::symlink(real.join(entry), shadow.join(entry))
            .map_err(|err| layout_failed(&shadow, &err))?;
    }
    let bin = layout.git_bin();
    write_file(
        &bin.join("git"),
        git_wrapper(&program, work, &shadow).as_bytes(),
        0o555,
    )?;
    Ok(Some(bin))
}

/// Das Agent-Programm: ein Verweis im Sitzungsverzeichnis auf die Datei, die
/// die Suche geprüft hat; lesbar wird nur sie. Gibt die einzeln lesbaren
/// Dateien zurück.
fn place_agent(
    session: &SessionContext,
    map: &PathMap,
    shim: &Path,
) -> Result<Vec<PathBuf>, Diagnostic> {
    let mut files = vec![shim.to_path_buf()];
    if let Some(agent) = &session.agent_program {
        if let Some(link) = map.map_path(&agent.dst) {
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(&agent.host, &link)
                .map_err(|err| layout_failed(&link, &err))?;
        }
        files.push(canonical(&agent.host)?);
    }
    Ok(files)
}

/// Ob ein Pfad unter dem Projekt liegt, als Pfad relativ zu ihm.
fn under_work(path: &Path) -> Option<PathBuf> {
    let rest = path.strip_prefix(WORK_DST).ok()?;
    rest.components()
        .all(|component| matches!(component, Component::Normal(_)))
        .then(|| rest.to_path_buf())
}

impl SandboxBackend for SeatbeltBackend {
    fn name(&self) -> &'static str {
        "seatbelt"
    }

    fn plan(
        &self,
        profile: &SandboxProfile,
        session: &SessionContext,
    ) -> Result<LaunchPlan, Diagnostic> {
        let work = self.check(profile, session)?;
        let layout = self.create_layout(session)?;
        // Kanonisch: Seatbelt vergleicht aufgelöste Pfade, und unter macOS
        // liegt das Laufzeitverzeichnis hinter dem Verweis `/var`.
        let door = canonical(&session.proxy_socket_src)?;
        let shim = canonical(&session.shim_src)?;
        let map = layout.path_map(&work, &door, &shim);
        let mut warnings = profile.unmask_diagnostics();
        place_files(session, &map, &mut warnings)?;
        let files = place_agent(session, &map, &shim)?;
        let port = session_port(None)?;
        let agent_port = session_port(Some(port))?;
        let unprotected = BwrapBackend::unprotected_under_work(
            profile,
            &BwrapBackend::present_under_work(profile, &work),
        );
        let policy = self.policy(
            profile,
            session,
            &work,
            &layout,
            files,
            &door,
            (port, agent_port),
        );
        let git_bin = place_git(&layout, &work, &policy.masked)?;
        let policy_text = policy.render();
        write_file(&layout.profile(), policy_text.as_bytes(), 0o444)?;

        let (report_reader, report_writer) = BwrapBackend::inherited_pipe("report")?;
        let (status_reader, status_writer) = BwrapBackend::inherited_pipe("status")?;
        let mut env = session_env(profile, session, &map, &layout, &door, port, &report_writer);
        env.push((
            crate::agent::ENV_AGENT_PORT.to_owned(),
            agent_port.to_string(),
        ));
        if let Some(bin) = git_bin {
            for (key, value) in &mut env {
                if key == "PATH" {
                    *value = format!("{}:{value}", bin.display());
                }
            }
        }
        let shells = self.shell_seed(&layout, &shim, &work, &env);

        let mut argv: Vec<OsString> = vec![
            self.program.clone().into_os_string(),
            "-f".into(),
            layout.profile().into_os_string(),
            shim.into_os_string(),
            "--proxy-port".into(),
            port.to_string().into(),
            "--status-fd".into(),
            status_writer.as_raw_fd().to_string().into(),
            "--chdir".into(),
            work.as_os_str().to_owned(),
        ];
        if self.stdio.pty_size().is_some() {
            argv.push("--controlling-terminal".into());
        }
        argv.push("--".into());
        argv.extend(
            session
                .command
                .iter()
                .map(|arg| OsString::from(map.map_text(&arg.to_string_lossy()))),
        );

        let inherit: Vec<OwnedFd> = vec![report_writer, status_writer];
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
            env_on_program: true,
            program: self.program.clone(),
            once: Mutex::new(Some(LaunchOnce {
                inherit,
                report: report_reader,
                status: status_reader,
                shells,
            })),
        })
    }

    fn launch(&self, plan: &LaunchPlan) -> Result<SandboxHandle, Diagnostic> {
        crate::bwrap::launch_plan(&self.program, plan, self.stdio, self.output_sink.clone())
    }

    fn isolation_check(&self, handle: &SandboxHandle) -> Vec<CheckResult> {
        crate::bwrap::isolation_from_report(handle, self.report_timeout)
    }
}

/// Die Umgebung des Agenten unter macOS: die des Profils, abgebildet, mit
/// eigenem Port, eigenem Suchpfad und der Tür als echtem Pfad.
fn session_env(
    profile: &SandboxProfile,
    session: &SessionContext,
    map: &PathMap,
    layout: &SessionLayout,
    door: &Path,
    port: u16,
    report: &OwnedFd,
) -> Vec<(String, String)> {
    let linux_door = format!("127.0.0.1:{}", profile.network.proxy_port);
    let own_door = format!("127.0.0.1:{port}");
    let mut env: std::collections::BTreeMap<String, String> = profile
        .effective_env(session, Some(report.as_raw_fd()))
        .into_iter()
        .map(|(key, value)| {
            let value = map.map_text(&value).replace(&linux_door, &own_door);
            (key, value)
        })
        .collect();
    let agent_dir = map
        .map_path(Path::new(crate::agent::AGENT_PROGRAM_DIR))
        .unwrap_or_else(|| layout.run().join("agent"));
    let path = if session.agent_program.is_some() {
        format!("{}:{SEARCH_PATH}", agent_dir.display())
    } else {
        SEARCH_PATH.to_owned()
    };
    env.insert("PATH".to_owned(), path);
    env.insert("TMPDIR".to_owned(), format!("{}/", layout.tmp().display()));
    env.insert(
        crate::bridge_env::ENV_BRIDGES.to_owned(),
        bridges_json(&[Bridge {
            name: crate::profile::PROXY_BRIDGE.to_owned(),
            dir: BridgeDirection::In,
            listen: SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
            socket: door.to_path_buf(),
        }]),
    );
    env.into_iter().collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn layout() -> SessionLayout {
        SessionLayout {
            root: PathBuf::from("/private/var/folders/x/T/humanitl-501/sessions/s1"),
        }
    }

    fn map() -> PathMap {
        layout().path_map(
            Path::new("/Users/me/project"),
            Path::new("/private/var/folders/x/T/humanitl-501/proxy/proxy.sock"),
            Path::new("/Applications/Humanitl.app/Contents/MacOS/bin/humanitl-shim"),
        )
    }

    #[test]
    fn sandbox_paths_land_in_the_session_or_the_project() {
        let map = map();
        assert_eq!(
            map.map_path(Path::new("/work/src/main.rs")).unwrap(),
            Path::new("/Users/me/project/src/main.rs")
        );
        assert_eq!(
            map.map_path(Path::new("/run/humanitl/proxy.sock")).unwrap(),
            Path::new("/private/var/folders/x/T/humanitl-501/proxy/proxy.sock"),
            "the door is the real socket, not a file in the session"
        );
        assert_eq!(
            map.map_path(Path::new("/etc/humanitl/pi/llm-api-key"))
                .unwrap(),
            layout().root.join("etc/humanitl/pi/llm-api-key")
        );
        assert_eq!(map.map_path(Path::new("/etc/opencode/opencode.json")), None);
        assert_eq!(map.map_path(Path::new("/workspace")), None);
        assert_eq!(map.map_path(Path::new("/tmp//etc")), None);
        assert_eq!(map.map_path(Path::new("/tmp/../etc")), None);
    }

    #[test]
    fn a_key_path_with_a_space_and_an_apostrophe_is_mapped_whole() {
        let root = "/Users/o'brien/Library/Application Support/Humanitl/s1/etc/humanitl";
        let map = PathMap::new([("/etc/humanitl".to_owned(), PathBuf::from(root))]);
        assert_eq!(
            map.map_text(crate::agent::pi::API_KEY_DST),
            format!("{root}/pi/llm-api-key")
        );
    }

    #[test]
    fn text_is_mapped_only_at_path_boundaries() {
        let map = map();
        assert_eq!(
            map.map_text("!cat /etc/humanitl/pi/llm-api-key"),
            format!(
                "!cat {}/etc/humanitl/pi/llm-api-key",
                layout().root.display()
            )
        );
        assert_eq!(
            map.map_text("cd /work && ls /workspace a/work"),
            "cd /Users/me/project && ls /workspace a/work"
        );
        assert_eq!(
            map.map_text("HOME=/home/agent:/tmp"),
            format!(
                "HOME={}:{}",
                layout().home().display(),
                layout().tmp().display()
            )
        );
        assert_eq!(
            map.map_text("ünïcode /work/ä"),
            "ünïcode /Users/me/project/ä"
        );
    }

    fn policy() -> SeatbeltPolicy {
        SeatbeltPolicy {
            work: PathBuf::from("/Users/me/project"),
            work_writable: true,
            session: layout().root,
            session_writable: vec![layout().home(), layout().tmp()],
            read_only: vec![],
            writable: vec![],
            files: vec![PathBuf::from("/Users/me/.opencode/bin/opencode")],
            masked: vec![PathBuf::from("/Users/me/project/.git/hooks")],
            locked: vec![PathBuf::from("/Users/me/project/.git")],
            door: PathBuf::from("/private/var/folders/x/T/humanitl-501/proxy/proxy.sock"),
            port: 41_234,
            agent_port: 41_235,
        }
    }

    #[test]
    fn the_session_config_keeps_the_format_and_nothing_else() {
        let real = "[core]\n\trepositoryformatversion = 1\n\tsshCommand = x\n\
                    \tfsmonitor = y\n\tignorecase = true\n\
                    [extensions]\n\tobjectformat = sha256\n\tpartialclone = origin\n\
                    [remote \"origin\"]\n\turl = https://token@example.com/x\n";
        let config = git_format_config(real);
        assert_eq!(
            config,
            "[core]\n\trepositoryformatversion = 1\n\tignorecase = true\n\
             [extensions]\n\tobjectformat = sha256\n"
        );
        assert_eq!(git_format_config("[core]\n\tignorecase = $(x)\n"), "");
    }

    #[test]
    fn git_gets_the_filtered_config_beside_the_real_objects() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let work = root.join("project");
        std::fs::create_dir_all(work.join(".git")).unwrap();
        std::fs::write(work.join(".git/config"), "[core]\n\tsshCommand = x\n").unwrap();
        let session = SessionLayout {
            root: root.join("session"),
        };
        let config = work.join(".git/config");

        assert!(place_git(&session, &work, &[]).unwrap().is_none());
        assert_eq!(
            place_git(&session, &work, &[config]).unwrap(),
            Some(session.git_bin())
        );
        assert_eq!(std::fs::read(session.git().join("config")).unwrap(), b"");
        assert_eq!(
            std::fs::read_link(session.git().join("objects")).unwrap(),
            work.join(".git/objects")
        );
    }

    #[test]
    fn the_wrapper_quotes_its_paths_and_leaves_clone_alone() {
        let script = git_wrapper(
            Path::new("/usr/bin/git"),
            Path::new("/Users/me/my project"),
            Path::new("/s/git"),
        );
        assert!(script.contains("'/Users/me/my project'"), "{script}");
        assert!(script.contains("clone|init) exec /usr/bin/git"), "{script}");
    }

    #[test]
    fn a_session_port_avoids_the_taken_and_a_v6_listener() {
        let first = session_port(None).unwrap();
        let v6 = TcpListener::bind((Ipv6Addr::LOCALHOST, 0)).unwrap();
        let busy = v6.local_addr().unwrap().port();
        let second = session_port(Some(first)).unwrap();
        assert_ne!(first, second);
        assert_ne!(second, busy);
        assert!(!AGENT_SERVICE_PORTS.contains(&second));
    }

    #[test]
    fn the_profile_denies_by_default_and_names_one_door() {
        let text = policy().render();
        assert!(text.contains("(deny default)"));
        // Der Zugangsschlüssel liegt unter macOS im Keychain (HUM-349); nur
        // diese Lücke in der Mach-Liste hält `security` von ihm fern.
        for service in MACH_SERVICES {
            let lower = service.to_lowercase();
            assert!(
                !lower.contains("secur") && !lower.contains("secd") && !lower.contains("keychain"),
                "{service} opens the Keychain to the agent"
            );
        }
        assert!(text.contains(
            "(allow network-outbound (remote unix-socket (path-literal \"/private/var/folders/x/T/humanitl-501/proxy/proxy.sock\")))"
        ));
        assert!(text.contains("(allow network-outbound (remote ip \"localhost:41234\"))"));
        assert!(text.contains("(allow network-bind (local ip \"localhost:41235\"))"));
        assert_eq!(text.matches("network-outbound").count(), 3, "{text}");
        assert_eq!(text.matches("network-bind").count(), 2, "{text}");
        assert!(!text.contains("(allow network*)"));
        assert!(text.contains("(literal \"/dev/stdout\")"));
        assert!(text.contains("(deny file-write* (literal \"/Users/me/project/.git\"))"));
        assert!(text.contains("(deny file-read-data file-write* (regex #\"^/dev/ttys[0-9]+$\"))"));
        // Die Masken stehen nach allen Erlaubnissen, denn die letzte passende
        // Regel gewinnt.
        let mask =
            text.find("(deny file-read* file-write* (subpath \"/Users/me/project/.git/hooks\"))");
        let last_allow = text.rfind("(allow file-write*");
        assert!(mask.unwrap() > last_allow.unwrap());
    }

    #[test]
    fn the_profile_and_the_adapter_files_are_not_writable() {
        let text = policy().render();
        let writes = text
            .lines()
            .find(|line| line.starts_with("(allow file-write*"))
            .unwrap();
        let root = layout().root.display().to_string();
        assert!(
            !writes.contains(&format!("(subpath \"{root}\")")),
            "{writes}"
        );
        assert!(
            writes.contains(&format!("(subpath \"{root}/home\")")),
            "{writes}"
        );
        assert!(
            writes.contains(&format!("(subpath \"{root}/tmp\")")),
            "{writes}"
        );
    }

    #[test]
    fn a_read_only_project_is_not_writable() {
        let mut policy = policy();
        policy.work_writable = false;
        let text = policy.render();
        let writes = text
            .lines()
            .find(|line| line.starts_with("(allow file-write*"))
            .unwrap();
        assert!(!writes.contains("/Users/me/project\""), "{writes}");
    }

    #[test]
    fn quotes_and_backslashes_cannot_end_a_literal() {
        assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
    }
}
