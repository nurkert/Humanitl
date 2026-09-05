//! Die Maschine lesen: aus Dateien, Verzeichnissen und vier kurzen Aufrufen
//! werden [`MachineFacts`].
//!
//! Hier steht alles, was Ein- und Ausgabe macht, und sonst nichts: Kein Urteil,
//! kein Befund, keine Schwelle. Was gemessen wurde, wandert als Wert in die
//! Tatsachen, und was nicht gemessen werden konnte, wandert als
//! [`Reading::Absent`] oder [`Reading::Unreadable`] dorthin — nie als
//! stillschweigende Voreinstellung.
//!
//! # Zwei Arten von Pfaden
//!
//! - **Aus der Umgebung**: `PATH`, `XDG_RUNTIME_DIR`, `XDG_DATA_HOME`,
//!   `LD_LIBRARY_PATH`. Sie werden genommen, wie sie dastehen. Ein Test setzt
//!   sie über [`Env`] auf ein eigenes Verzeichnis.
//! - **Vom Modul selbst benannt**: `/proc/self/status`, `/etc/ld.so.conf.d`,
//!   `/usr/lib`. Sie liegen unter [`Probe::with_root`], das auf einem Rechner
//!   `/` ist und im Test ein Verzeichnis mit denselben Namen darunter. Auch
//!   was aus einer solchen Datei gelesen wird — die Zeilen von `ld.so.conf` —
//!   bekommt die Wurzel vorangestellt: Es gehört zu demselben vorgetäuschten
//!   System.
//!
//! # Fristen
//!
//! Jeder Aufruf hat eine Frist ([`DEFAULT_TIMEOUT`]). Auf gehärteten Systemen
//! bleibt eine Namensraum-Probe hängen, statt zu scheitern (HUM-075,
//! Fallstricke); ohne Frist bliebe der Doctor mit ihr hängen. Ausgabe und
//! Fehlerausgabe der Aufrufe gehen in je ein `memfd` und nicht in eine Pipe:
//! Eine Pipe mit vollem Puffer wäre der zweite Weg, auf dem derselbe Aufruf
//! hängen bleibt.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{ErrorKind, Read, Seek, SeekFrom};
use std::os::fd::OwnedFd;
use std::os::unix::fs::MetadataExt as _;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use humanitl_config::{Env, Paths};
use rustix::fs::{Access, MemfdFlags, access, memfd_create};

use super::{
    AgentFacts, BwrapFacts, CommandRun, DaemonFacts, DiskFacts, LlmFacts, MachineFacts, Reading,
    RendererFacts, RunOutcome, RuntimeDirFacts, SeccompFacts, SeccompLine, SystemdFacts, TrayFacts,
    UsernsFacts,
};
use crate::agent::opencode;
use crate::bwrap::{BwrapBackend, Version};

/// Wie lange ein einzelner Aufruf des Doctors höchstens laufen darf.
///
/// Zwei Sekunden, wie HUM-075 sie für die Namensraum-Probe nennt; dieselbe
/// Frist gilt für alle vier Aufrufe, damit ein hängendes `systemctl` den
/// Doctor genauso wenig aufhält.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(2);

/// Ab wie viel freiem Platz im Datenverzeichnis die Zeile grün ist.
pub const MIN_FREE_BYTES: u64 = 1024 * 1024 * 1024;

/// Wo `bwrap` und `systemctl` gesucht werden, wenn `PATH` fehlt.
///
/// Dieselbe Liste wie in [`crate::bwrap`]; sie steht hier ein zweites Mal, weil
/// der Doctor den durchsuchten `PATH` in seinen Beleg schreibt und ihn dafür
/// selbst kennen muss. Der Test `the_doctor_searches_where_the_launcher_searches`
/// hält beide zusammen.
const DEFAULT_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Die Verzeichnisse, in denen der dynamische Lader ohne Konfiguration sucht.
const DEFAULT_LIBRARY_DIRS: &[&str] = &["/lib", "/usr/lib", "/lib64", "/usr/lib64"];

/// Die Anfänge der Dateinamen, unter denen die Tray-Bibliothek liegt.
///
/// Beide zählen: Die ältere `libappindicator3` tut dasselbe und ist auf
/// manchen Systemen die einzige.
const TRAY_LIBRARIES: &[&str] = &["libayatana-appindicator3.so", "libappindicator3.so"];

/// So viele Bytes einer Programmausgabe liest der Doctor höchstens.
const OUTPUT_CAP_BYTES: u64 = 64 * 1024;

/// Wie oft zwischen zwei Blicken auf ein laufendes Kind gewartet wird.
const POLL_INTERVAL: Duration = Duration::from_millis(10);

/// Die Variablen, die ein Aufruf mit [`RunEnv::Session`] mitbekommt.
///
/// Nicht die ganze Umgebung: In ihr stehen Tokens und Schlüssel des Nutzers,
/// und sie haben in einem Programm nichts zu suchen, das der Doctor startet.
/// Nicht die leere Umgebung: `systemctl --user` findet ohne
/// `$XDG_RUNTIME_DIR` und `$DBUS_SESSION_BUS_ADDRESS` den Bus der Sitzung
/// nicht und antwortet „Failed to connect to user scope bus", und das
/// Startskript von `OpenCode` bricht ohne `$HOME` mit `unbound variable` ab —
/// beides sah nach einer kaputten Maschine aus, war aber der Doctor.
///
/// Die drei Lader-Variablen (`LD_PRELOAD`, `LD_AUDIT`, `LD_LIBRARY_PATH`)
/// stehen mit Absicht nicht darunter (`humanitl_config::is_loader_key`).
const SESSION_ENV_KEYS: &[&str] = &[
    "PATH",
    "HOME",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
];

/// Mit welcher Umgebung ein Aufruf startet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunEnv {
    /// Keine einzige Variable, wie in [`BwrapBackend::scrubbed_command`].
    ///
    /// Für jeden Start von `bwrap`. `bwrap` bleibt als PID 1 des Namensraums
    /// stehen, und `/proc/1/environ` zeigte sonst die Umgebung des Nutzers
    /// (ESC-2-Befund, `backlog/CONVENTIONS.md` 4.11) — auch bei einer Probe,
    /// die nur `/bin/true` startet.
    Empty,
    /// Die wenigen Variablen aus [`SESSION_ENV_KEYS`].
    Session,
}

/// Die Argumente der Namensraum-Probe.
///
/// Wortgleich mit [`BwrapBackend::probe_user_namespaces`], und mit Absicht
/// nicht dieselbe Funktion: Die dortige Probe hat keine Frist und wertet nur
/// die bekannten Fehlerbilder als Befund, weil dort gleich darauf ein echter
/// Start folgt. Der Doctor braucht das rohe Ergebnis samt Frist, um zwischen
/// „verweigert", „hängt" und „aus einem anderen Grund gescheitert"
/// unterscheiden zu können.
const USERNS_ARGS: &[&str] = &[
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
];

/// Liest die Maschine.
#[derive(Debug, Clone)]
pub struct Probe<'a> {
    env: &'a Env,
    root: PathBuf,
    timeout: Duration,
    adapter: String,
    agent_command: Option<String>,
}

impl<'a> Probe<'a> {
    /// Eine Probe über dieser Umgebung, mit `/` als Wurzel der Systemdateien.
    #[must_use]
    pub fn new(env: &'a Env) -> Self {
        Self {
            env,
            root: PathBuf::from("/"),
            timeout: DEFAULT_TIMEOUT,
            adapter: opencode::ADAPTER_ID.to_owned(),
            agent_command: None,
        }
    }

    /// Legt die Wurzel der Systemdateien fest; für Tests.
    ///
    /// Betrifft nur die Pfade, die dieses Modul selbst benennt, nie die aus
    /// der Umgebung.
    #[must_use]
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.root = root.into();
        self
    }

    /// Legt die Frist je Aufruf fest.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Legt fest, welcher Agent gesucht wird.
    ///
    /// `adapter` ist `agent.adapter`, `command` das erste Wort von
    /// `agent.command`, wenn die Konfiguration die Kommandozeile ersetzt.
    #[must_use]
    pub fn with_agent(mut self, adapter: impl Into<String>, command: Option<String>) -> Self {
        self.adapter = adapter.into();
        self.agent_command = command;
        self
    }

    /// Liest alles, was diese Maschine über sich preisgibt.
    ///
    /// `daemon` und `llm` kommen vom Aufrufer: Über den Daemon weiß der
    /// Aufrufer Bescheid, weil er gerade mit ihm gesprochen hat oder es
    /// vergeblich versucht hat, und der Endpunkt des Sprachmodells wird hier
    /// grundsätzlich nicht angefasst (siehe [`super`]).
    ///
    /// Die vier Aufrufe laufen nebeneinander, damit der ganze Doctor unter
    /// einer Frist bleibt und nicht unter vieren.
    #[must_use]
    pub fn collect(&self, daemon: DaemonFacts, llm: LlmFacts) -> MachineFacts {
        let bwrap_program = BwrapBackend::find_program(self.env).ok();
        let systemctl = self.find_executable("systemctl");
        let agent_command = self
            .agent_command
            .clone()
            .unwrap_or_else(|| default_command(&self.adapter));
        let agent_program = self.find_executable(&agent_command);

        let (bwrap_version, userns_probe, systemd_state, agent_version) =
            std::thread::scope(|scope| {
                let version = scope.spawn(|| {
                    bwrap_program
                        .as_ref()
                        .map(|program| self.run(program, &["--version"], RunEnv::Empty))
                });
                let userns = scope.spawn(|| {
                    bwrap_program
                        .as_ref()
                        .map(|program| self.run(program, USERNS_ARGS, RunEnv::Empty))
                });
                let systemd = scope.spawn(|| {
                    systemctl.as_ref().map(|program| {
                        self.run(program, &["--user", "is-system-running"], RunEnv::Session)
                    })
                });
                let agent = scope.spawn(|| {
                    agent_program
                        .as_ref()
                        .map(|program| self.run(program, &["--version"], RunEnv::Session))
                });
                (
                    version.join().ok().flatten(),
                    userns.join().ok().flatten(),
                    systemd.join().ok().flatten(),
                    agent.join().ok().flatten(),
                )
            });

        MachineFacts {
            bwrap: bwrap_facts(bwrap_program.as_deref(), bwrap_version, self.path()),
            userns: UsernsFacts {
                probe: reading(userns_probe),
                apparmor_restrict: self
                    .read_file("/proc/sys/kernel/apparmor_restrict_unprivileged_userns"),
                userns_clone: self.read_file("/proc/sys/kernel/unprivileged_userns_clone"),
            },
            seccomp: SeccompFacts {
                line: self.seccomp_line(),
                kernel_release: self.read_file("/proc/sys/kernel/osrelease"),
            },
            runtime_dir: self.runtime_dir(),
            systemd_user: SystemdFacts {
                state: reading(systemd_state),
                searched: self.path().to_owned(),
            },
            daemon,
            agent: agent_facts(
                &self.adapter,
                &agent_command,
                agent_program.as_deref(),
                agent_version,
                self.path(),
            ),
            llm,
            tray: self.tray(),
            renderer: self.renderer(),
            disk: self.disk(),
        }
    }

    /// Der `PATH`, in dem gesucht wird.
    fn path(&self) -> &str {
        self.env.non_empty("PATH").unwrap_or(DEFAULT_PATH)
    }

    /// Ein Pfad, den dieses Modul selbst benennt, unter der Wurzel.
    fn under(&self, path: &str) -> PathBuf {
        if self.root == Path::new("/") {
            return PathBuf::from(path);
        }
        self.root.join(path.trim_start_matches('/'))
    }

    /// Sucht ein ausführbares Programm im `PATH` der Umgebung.
    ///
    /// Ein Name mit Trennzeichen ist ein Pfad und wird nicht gesucht; so wirkt
    /// ein `agent.command`, das schon absolut ist.
    fn find_executable(&self, name: &str) -> Option<PathBuf> {
        let candidate = Path::new(name);
        if candidate.components().count() > 1 {
            return (candidate.is_file() && access(candidate, Access::EXEC_OK).is_ok())
                .then(|| candidate.to_path_buf());
        }
        self.path()
            .split(':')
            .filter(|dir| !dir.is_empty())
            .map(|dir| Path::new(dir).join(name))
            .find(|full| full.is_file() && access(full, Access::EXEC_OK).is_ok())
    }

    /// Liest eine Datei, die dieses Modul selbst benennt.
    fn read_file(&self, path: &str) -> Reading<String> {
        read_at(&self.under(path))
    }

    /// Die Zeile `Seccomp:` aus `/proc/self/status`.
    fn seccomp_line(&self) -> Reading<SeccompLine> {
        match read_at(&self.under("/proc/self/status")) {
            Reading::Found(text) => Reading::Found(
                text.lines()
                    .find_map(|line| line.strip_prefix("Seccomp:"))
                    .map_or(SeccompLine::Missing, |value| {
                        SeccompLine::Present(value.trim().to_owned())
                    }),
            ),
            Reading::Absent => Reading::Absent,
            Reading::Unreadable(error) => Reading::Unreadable(error),
        }
    }

    /// Was `$XDG_RUNTIME_DIR` benennt und was dort liegt.
    ///
    /// Gemeint ist das Verzeichnis aus der Umgebung selbst, nicht das
    /// `humanitl`-Unterverzeichnis darin: Über dessen Rechte entscheidet der
    /// Daemon beim Anlegen ([`humanitl_config::DIR_MODE`]), über die des
    /// Elternverzeichnisses die Anmeldung.
    fn runtime_dir(&self) -> RuntimeDirFacts {
        let Some(dir) = self.env.non_empty("XDG_RUNTIME_DIR") else {
            return RuntimeDirFacts::Unset {
                expected: PathBuf::from(format!("/run/user/{}", self.env.uid())),
            };
        };
        let path = PathBuf::from(dir);
        match std::fs::metadata(&path) {
            Ok(meta) => RuntimeDirFacts::Present {
                mode: meta.permissions().mode() & 0o777,
                owner_uid: meta.uid(),
                our_uid: self.env.uid(),
                is_dir: meta.is_dir(),
                path,
            },
            Err(error) if error.kind() == ErrorKind::NotFound => RuntimeDirFacts::Missing { path },
            Err(error) => RuntimeDirFacts::Unreadable {
                path,
                error: error.to_string(),
            },
        }
    }

    /// Die Verzeichnisse, in denen der dynamische Lader sucht.
    ///
    /// `$LD_LIBRARY_PATH` kommt aus der Umgebung und bleibt, wie es dasteht;
    /// alles aus `/etc/ld.so.conf` und die eingebauten Vorgaben liegen unter
    /// der Wurzel.
    fn library_dirs(&self) -> Vec<PathBuf> {
        let mut dirs: Vec<PathBuf> = Vec::new();
        let mut seen: BTreeSet<PathBuf> = BTreeSet::new();
        let push = |dir: PathBuf, dirs: &mut Vec<PathBuf>, seen: &mut BTreeSet<PathBuf>| {
            if seen.insert(dir.clone()) {
                dirs.push(dir);
            }
        };

        for dir in self
            .env
            .non_empty("LD_LIBRARY_PATH")
            .unwrap_or_default()
            .split(':')
            .filter(|dir| !dir.is_empty())
        {
            push(PathBuf::from(dir), &mut dirs, &mut seen);
        }

        for conf in self.loader_configs() {
            let Reading::Found(text) = read_at(&conf) else {
                continue;
            };
            for line in text.lines() {
                let line = line.split('#').next().unwrap_or_default().trim();
                if line.is_empty() || line.starts_with("include") {
                    continue;
                }
                push(self.under(line), &mut dirs, &mut seen);
            }
        }

        for dir in DEFAULT_LIBRARY_DIRS {
            push(self.under(dir), &mut dirs, &mut seen);
        }
        if let Some(triple) = multiarch_triple() {
            push(
                self.under(&format!("/usr/lib/{triple}")),
                &mut dirs,
                &mut seen,
            );
            push(self.under(&format!("/lib/{triple}")), &mut dirs, &mut seen);
        }
        dirs
    }

    /// `/etc/ld.so.conf` und alles unter `/etc/ld.so.conf.d`.
    fn loader_configs(&self) -> Vec<PathBuf> {
        let mut configs = vec![self.under("/etc/ld.so.conf")];
        let Ok(entries) = std::fs::read_dir(self.under("/etc/ld.so.conf.d")) else {
            return configs;
        };
        let mut extra: Vec<PathBuf> = entries
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "conf"))
            .collect();
        extra.sort();
        configs.append(&mut extra);
        configs
    }

    /// Sucht die Tray-Bibliothek in den Verzeichnissen des Laders.
    fn tray(&self) -> TrayFacts {
        let dirs = self.library_dirs();
        let mut readable_dirs = 0;
        let mut library = None;
        for dir in &dirs {
            let Ok(entries) = std::fs::read_dir(dir) else {
                continue;
            };
            readable_dirs += 1;
            if library.is_some() {
                continue;
            }
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if TRAY_LIBRARIES.iter().any(|lib| name.starts_with(lib)) {
                    library = Some(entry.path());
                    break;
                }
            }
        }
        TrayFacts {
            library,
            readable_dirs,
            searched_dirs: dirs.len(),
            desktop: self.env.non_empty("XDG_CURRENT_DESKTOP").map(str::to_owned),
        }
    }

    /// Sitzungsart und Grafiktreiber.
    fn renderer(&self) -> RendererFacts {
        RendererFacts {
            session_type: self.env.non_empty("XDG_SESSION_TYPE").map(str::to_owned),
            nvidia: match self.read_file("/proc/modules") {
                Reading::Found(text) => Reading::Found(
                    text.lines()
                        .any(|line| line.split_whitespace().next().is_some_and(is_nvidia_module)),
                ),
                Reading::Absent => Reading::Absent,
                Reading::Unreadable(error) => Reading::Unreadable(error),
            },
            flutter_engine: self.env.non_empty("FLUTTER_ENGINE").map(str::to_owned),
        }
    }

    /// Der freie Platz im Datenverzeichnis.
    ///
    /// Gemessen wird am ersten Vorfahren, den es schon gibt: Vor der ersten
    /// Sitzung gibt es `$XDG_DATA_HOME/humanitl` noch nicht, und der freie
    /// Platz seines Dateisystems ist trotzdem die Zahl, um die es geht.
    fn disk(&self) -> DiskFacts {
        let wanted = Paths::new(self.env.clone()).data_dir();
        let mut path = wanted.as_path();
        loop {
            match rustix::fs::statvfs(path) {
                Ok(stat) => {
                    return DiskFacts::Measured {
                        path: path.to_path_buf(),
                        available_bytes: stat.f_bavail.saturating_mul(stat.f_frsize),
                    };
                }
                Err(error) => {
                    let Some(parent) = path.parent() else {
                        return DiskFacts::Unreadable {
                            path: wanted.clone(),
                            error: error.to_string(),
                        };
                    };
                    path = parent;
                }
            }
        }
    }

    /// Startet ein Programm mit leerer Umgebung und einer Frist.
    ///
    /// Wie viel von der Umgebung mitgeht, sagt `env` ([`RunEnv`]): `bwrap`
    /// bekommt nichts, `systemctl --user` und der Agent die vier Variablen
    /// aus [`SESSION_ENV_KEYS`], ohne die sie ihre eigene Sitzung nicht
    /// finden. Ausgabe und Fehlerausgabe landen in je einem `memfd`, damit
    /// kein voller Pipe-Puffer den Aufruf anhalten kann.
    fn run(&self, program: &Path, args: &[&str], env: RunEnv) -> Result<CommandRun, String> {
        let words: Vec<String> = std::iter::once(program.to_string_lossy().into_owned())
            .chain(args.iter().map(|arg| (*arg).to_owned()))
            .collect();

        let (Ok(mut out), Ok(mut err)) = (scratch_file(), scratch_file()) else {
            return Err("no memfd for the output of the call".to_owned());
        };
        let (Ok(out_handle), Ok(err_handle)) = (out.try_clone(), err.try_clone()) else {
            return Err("the output file could not be duplicated".to_owned());
        };

        let mut command = Command::new(program);
        command.args(args).env_clear();
        if env == RunEnv::Session {
            for key in SESSION_ENV_KEYS {
                if let Some(value) = self.env.non_empty(key) {
                    command.env(key, value);
                }
            }
        }
        let mut child = match command
            .stdin(Stdio::null())
            .stdout(Stdio::from(out_handle))
            .stderr(Stdio::from(err_handle))
            .spawn()
        {
            Ok(child) => child,
            Err(error) => return Err(error.to_string()),
        };

        let started = Instant::now();
        let outcome = loop {
            match child.try_wait() {
                Ok(Some(status)) => break exit_status(status),
                Err(error) => {
                    let _ = child.kill();
                    return Err(error.to_string());
                }
                Ok(None) => {}
            }
            if started.elapsed() >= self.timeout {
                let _ = child.kill();
                let _ = child.wait();
                break RunOutcome::TimedOut(self.timeout);
            }
            std::thread::sleep(POLL_INTERVAL);
        };

        Ok(CommandRun::new(
            words,
            outcome,
            read_back(&mut out)?,
            read_back(&mut err)?,
        ))
    }
}

/// Wie ein beendetes Kind ausgegangen ist.
fn exit_status(status: std::process::ExitStatus) -> RunOutcome {
    use std::os::unix::process::ExitStatusExt as _;

    status.code().map_or_else(
        || RunOutcome::Signalled(status.signal().unwrap_or(0)),
        RunOutcome::Exited,
    )
}

/// Ein anonymer, seekbarer Puffer für die Ausgabe eines Aufrufs.
fn scratch_file() -> Result<File, rustix::io::Errno> {
    memfd_create("humanitl-doctor", MemfdFlags::CLOEXEC).map(|fd: OwnedFd| File::from(fd))
}

/// Liest den Puffer eines Aufrufs von vorn, höchstens [`OUTPUT_CAP_BYTES`].
///
/// **Streng, nicht `from_utf8_lossy`.** Ein `bwrap`, das mit 0 endet und
/// `\xffbubblewrap 0.9.1` schreibt, hätte mit dem verlustbehafteten Weg als
/// gemessene Fassung gegolten — die Bytes waren nicht lesbar, und die Prüfung
/// hätte trotzdem behauptet, sie habe gelesen. Unlesbare Bytes sind hier ein
/// Fehler und werden zu [`Reading::Unreadable`].
///
/// # Errors
///
/// Ein Satz, wenn sich der Puffer nicht lesen lässt oder kein `UTF-8` ist.
fn read_back(file: &mut File) -> Result<String, String> {
    file.seek(SeekFrom::Start(0))
        .map_err(|error| format!("the output of the call could not be rewound: {error}"))?;
    let mut buffer = Vec::new();
    file.take(OUTPUT_CAP_BYTES)
        .read_to_end(&mut buffer)
        .map_err(|error| format!("the output of the call could not be read: {error}"))?;
    let text = String::from_utf8(buffer)
        .map_err(|error| format!("the output of the call is not valid UTF-8: {error}"))?;
    Ok(text.trim().to_owned())
}

/// Liest eine Datei und unterscheidet „gibt es nicht" von „geht nicht".
fn read_at(path: &Path) -> Reading<String> {
    match std::fs::read_to_string(path) {
        Ok(text) => Reading::Found(text.trim().to_owned()),
        Err(error) if error.kind() == ErrorKind::NotFound => Reading::Absent,
        Err(error) => Reading::Unreadable(error.to_string()),
    }
}

/// Ein Aufruf als [`Reading`].
///
/// Drei Faelle, drei Antworten: Es gab kein Programm ([`Reading::Absent`]),
/// das Programm liess sich nicht starten ([`Reading::Unreadable`]), oder es
/// lief ([`Reading::Found`]). Ein erfundener Exit-Code fuer den mittleren
/// Fall waere eine Messung, die es nicht gab.
fn reading(run: Option<Result<CommandRun, String>>) -> Reading<CommandRun> {
    match run {
        None => Reading::Absent,
        Some(Ok(run)) => Reading::Found(run),
        Some(Err(error)) => Reading::Unreadable(error),
    }
}

/// Das Kommando, das ein Adapter ohne Ersatz aus der Konfiguration startet.
///
/// Der einzige eingebaute Adapter ist `opencode`
/// ([`crate::AdapterRegistry::builtin`]); ein unbekannter Name aus
/// `agent.adapter` wird als Kommando genommen, damit der Beleg wenigstens
/// sagt, wonach gesucht wurde.
fn default_command(adapter: &str) -> String {
    if adapter == opencode::ADAPTER_ID {
        opencode::DEFAULT_COMMAND.to_owned()
    } else {
        adapter.to_owned()
    }
}

/// Wie sich das Kommando eines Adapters nachinstallieren lässt.
fn install_command(adapter: &str) -> String {
    if adapter == opencode::ADAPTER_ID {
        opencode::INSTALL_COMMAND.to_owned()
    } else {
        format!("install {adapter} and put it in PATH")
    }
}

/// Die Tatsachen über `bwrap` aus Fund und Fassungsabfrage.
fn bwrap_facts(
    program: Option<&Path>,
    version: Option<Result<CommandRun, String>>,
    searched: &str,
) -> BwrapFacts {
    let Some(program) = program else {
        return BwrapFacts::Missing {
            searched: searched.to_owned(),
        };
    };
    let program = program.to_path_buf();
    let run = match version {
        Some(Ok(run)) => run,
        Some(Err(error)) => return BwrapFacts::Unreadable { program, error },
        None => {
            return BwrapFacts::Unreadable {
                program,
                error: "the call was not made".to_owned(),
            };
        }
    };
    if !run.outcome.is_success() {
        return BwrapFacts::Unreadable {
            program,
            error: format!("{}: {}", run.outcome.describe(), run.first_message()),
        };
    }
    match parse_version(&run.stdout) {
        Ok(version) => BwrapFacts::Found { program, version },
        Err(error) => BwrapFacts::Unreadable { program, error },
    }
}

/// Liest die Fassung aus der Antwort von `bwrap --version`, oder gar nicht.
///
/// [`Version::parse`] macht aus allem, was es nicht lesen kann, eine Null:
/// `bubblewrap 999999999999999999.9` ergibt dort `0.9.0` und besteht damit die
/// Mindestprüfung. Der Doctor bescheinigte so eine ausreichende Fassung, die
/// er gar nicht kennt. Dieser Weg liest dieselben Zahlengruppen, gibt aber
/// auf, sobald eine davon nicht in ein `u32` passt oder gar keine da ist.
/// `bwrap.rs` bleibt unberührt: Der Launcher hat seinen eigenen Aufrufer.
///
/// # Errors
///
/// Ein Satz, wenn die Antwort keine Zahl trägt oder eine Zahl zu groß ist.
fn parse_version(text: &str) -> Result<Version, String> {
    let mut numbers = Vec::new();
    for group in text.split(|c: char| !c.is_ascii_digit()) {
        if group.is_empty() {
            continue;
        }
        let number: u32 = group.parse().map_err(|_| {
            format!("--version answered {text:?}, and {group} is not a version number")
        })?;
        numbers.push(number);
        if numbers.len() == 3 {
            break;
        }
    }
    let mut parts = numbers.into_iter();
    let major = parts
        .next()
        .ok_or_else(|| format!("--version answered {text:?}, which carries no number"))?;
    Ok(Version(
        major,
        parts.next().unwrap_or(0),
        parts.next().unwrap_or(0),
    ))
}

/// Die Tatsachen über das Kommando des Agenten.
fn agent_facts(
    adapter: &str,
    command: &str,
    program: Option<&Path>,
    version: Option<Result<CommandRun, String>>,
    searched: &str,
) -> AgentFacts {
    let Some(program) = program else {
        return AgentFacts::Missing {
            adapter: adapter.to_owned(),
            command: command.to_owned(),
            searched: searched.to_owned(),
            install: install_command(adapter),
        };
    };
    let version = match version {
        None => Reading::Absent,
        Some(Err(error)) => Reading::Unreadable(error),
        Some(Ok(run)) if run.outcome.is_success() && !run.stdout.is_empty() => Reading::Found(
            run.stdout
                .lines()
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned(),
        ),
        Some(Ok(run)) => Reading::Unreadable(format!(
            "{}: {}",
            run.outcome.describe(),
            run.first_message()
        )),
    };
    AgentFacts::Found {
        adapter: adapter.to_owned(),
        command: command.to_owned(),
        program: program.to_path_buf(),
        version,
    }
}

/// Wahr, wenn ein Modulname aus `/proc/modules` der NVIDIA-Treiber ist.
///
/// `nvidia`, `nvidia_drm`, `nvidia_modeset`, `nvidia_uvm` zählen;
/// `nvidiafb`, der Framebuffer-Treiber des Kernels, nicht — er hat mit
/// Impeller nichts zu tun.
fn is_nvidia_module(name: &str) -> bool {
    name == "nvidia" || name.starts_with("nvidia_")
}

/// Das Multiarch-Tripel dieser Übersetzung, für `/usr/lib/<tripel>`.
///
/// Abgeleitet aus der Architektur des Binaries, nicht aus einem Pfad dieser
/// Maschine: Auf einem System, das `/etc/ld.so.conf.d` führt, steht das
/// Verzeichnis ohnehin dort; hier steht es für die anderen.
fn multiarch_triple() -> Option<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Some("x86_64-linux-gnu"),
        "aarch64" => Some("aarch64-linux-gnu"),
        "arm" => Some("arm-linux-gnueabihf"),
        "riscv64" => Some("riscv64-linux-gnu"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::PathBuf;
    use std::time::Duration;

    use humanitl_config::Env;

    use super::super::{BwrapFacts, DaemonFacts, LlmFacts, Reading, RuntimeDirFacts, SeccompLine};
    use super::{DEFAULT_PATH, Probe, is_nvidia_module, parse_version};
    use crate::bwrap::{BwrapBackend, Version};

    /// Eine Probe mit einer Frist, die eine ausgelastete Maschine nicht reisst.
    ///
    /// [`DEFAULT_TIMEOUT`] sind zwei Sekunden. Ein Testskript braucht davon
    /// Millisekunden — aber am 2026-09-05 lief dieser Rechner mit drei
    /// haengenden Testbinaries eines anderen Baums, und eine Probe fiel in
    /// ihre Frist. Ein Test, der die Auslastung der Maschine misst statt des
    /// Verhaltens, ist keiner. Wer die Frist selbst prueft, setzt sie danach
    /// mit `with_timeout` wieder kurz.
    fn probe(env: &Env) -> Probe<'_> {
        Probe::new(env).with_timeout(Duration::from_secs(20))
    }

    /// Ein ausführbares Skript, das `stdout`, `stderr` und Exit-Code setzt.
    fn script(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        use std::io::Write as _;
        use std::os::unix::fs::PermissionsExt as _;

        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "#!/bin/sh\n{body}").unwrap();
        drop(file);
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn env_with(pairs: &[(&str, &str)]) -> Env {
        Env::from_pairs(
            pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned())),
        )
        .with_uid(4242)
    }

    fn nothing() -> (DaemonFacts, LlmFacts) {
        (
            DaemonFacts::NotTried {
                socket: PathBuf::from("/nowhere/daemon.sock"),
                why: "this is a test".to_owned(),
            },
            LlmFacts::NoEndpoint,
        )
    }

    #[test]
    fn a_fake_bwrap_in_path_is_found_and_its_version_read() {
        let dir = tempfile::tempdir().unwrap();
        script(dir.path(), "bwrap", "echo 'bubblewrap 0.9.1'");
        let env = env_with(&[("PATH", &dir.path().display().to_string())]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);

        let BwrapFacts::Found { version, program } = facts.bwrap else {
            panic!("a fake bwrap in PATH must be found: {:?}", facts.bwrap);
        };
        assert_eq!(version, Version(0, 9, 1));
        assert_eq!(program, dir.path().join("bwrap"));
    }

    #[test]
    fn a_bwrap_that_prints_no_number_is_unreadable_and_not_version_zero() {
        let dir = tempfile::tempdir().unwrap();
        script(dir.path(), "bwrap", "echo 'command not found'");
        let env = env_with(&[("PATH", &dir.path().display().to_string())]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);

        let BwrapFacts::Unreadable { error, .. } = facts.bwrap else {
            panic!(
                "an answer without a number is no version: {:?}",
                facts.bwrap
            );
        };
        assert!(error.contains("no number"), "{error}");
    }

    #[test]
    fn an_empty_path_finds_no_bwrap_and_says_where_it_looked() {
        let env = env_with(&[("PATH", "/does/not/exist")]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);
        assert_eq!(
            facts.bwrap,
            BwrapFacts::Missing {
                searched: "/does/not/exist".to_owned()
            }
        );
    }

    #[test]
    fn the_doctor_searches_where_the_launcher_searches() {
        // Ohne PATH nimmt der Launcher seine eigene Vorgabe; der Doctor muss
        // dieselbe in den Beleg schreiben, sonst nennt er ein Verzeichnis, in
        // dem niemand gesucht hat.
        let dir = tempfile::tempdir().unwrap();
        script(dir.path(), "bwrap", "echo 'bubblewrap 0.9.1'");
        let env = env_with(&[("PATH", &dir.path().display().to_string())]);
        let found = BwrapBackend::find_program(&env).expect("the fake bwrap");
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);
        let BwrapFacts::Found { program, .. } = facts.bwrap else {
            panic!("both sides must find the same bwrap");
        };
        assert_eq!(program, found);
        assert!(DEFAULT_PATH.contains("/usr/bin"), "{DEFAULT_PATH}");
    }

    #[test]
    fn a_hanging_call_is_cut_off_and_reported_as_a_timeout() {
        let dir = tempfile::tempdir().unwrap();
        script(dir.path(), "bwrap", "sleep 30");
        let env = env_with(&[("PATH", &dir.path().display().to_string())]);
        let (daemon, llm) = nothing();
        let started = std::time::Instant::now();
        let facts = probe(&env)
            .with_timeout(Duration::from_millis(150))
            .collect(daemon, llm);
        // Grosszuegig: Gemessen wird, dass die Probe den Aufruf abschneidet
        // und nicht dessen dreissig Sekunden mitgeht — nicht, wie schnell
        // dieser Rechner gerade ist.
        assert!(
            started.elapsed() < Duration::from_secs(20),
            "the probe waited {:?} instead of cutting the call off",
            started.elapsed()
        );
        let run = facts.userns.probe.found().expect("a run");
        assert_eq!(
            run.outcome,
            super::RunOutcome::TimedOut(Duration::from_millis(150))
        );
    }

    #[test]
    fn kernel_files_are_read_below_the_root_and_absence_is_not_a_bad_value() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("proc/sys/kernel")).unwrap();
        std::fs::write(
            root.path()
                .join("proc/sys/kernel/apparmor_restrict_unprivileged_userns"),
            "1\n",
        )
        .unwrap();
        std::fs::write(root.path().join("proc/sys/kernel/osrelease"), "6.1.0-18\n").unwrap();
        std::fs::write(root.path().join("proc/self_status_placeholder"), "x").unwrap();
        std::fs::create_dir_all(root.path().join("proc/self")).unwrap();
        std::fs::write(
            root.path().join("proc/self/status"),
            "Name:\tsh\nSeccomp:\t2\nSeccomp_filters:\t1\n",
        )
        .unwrap();
        std::fs::write(root.path().join("proc/modules"), "nvidia_drm 1 0 - Live\n").unwrap();

        let env = env_with(&[("PATH", "/does/not/exist")]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).with_root(root.path()).collect(daemon, llm);

        assert_eq!(
            facts.userns.apparmor_restrict,
            Reading::Found("1".to_owned())
        );
        // Diese Datei gibt es unter der Wurzel nicht; das ist etwas anderes
        // als ein schlechter Wert.
        assert_eq!(facts.userns.userns_clone, Reading::Absent);
        assert_eq!(
            facts.seccomp.line,
            Reading::Found(SeccompLine::Present("2".to_owned()))
        );
        assert_eq!(
            facts.seccomp.kernel_release,
            Reading::Found("6.1.0-18".to_owned())
        );
        assert_eq!(facts.renderer.nvidia, Reading::Found(true));
    }

    #[test]
    fn a_status_without_a_seccomp_line_is_missing_and_not_absent() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("proc/self")).unwrap();
        std::fs::write(root.path().join("proc/self/status"), "Name:\tsh\n").unwrap();
        let env = env_with(&[("PATH", "/does/not/exist")]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).with_root(root.path()).collect(daemon, llm);
        assert_eq!(facts.seccomp.line, Reading::Found(SeccompLine::Missing));
    }

    #[test]
    fn the_runtime_dir_is_read_from_the_environment_with_its_mode() {
        let dir = tempfile::tempdir().unwrap();
        let env = env_with(&[
            ("PATH", "/does/not/exist"),
            ("XDG_RUNTIME_DIR", &dir.path().display().to_string()),
        ]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);
        let RuntimeDirFacts::Present {
            path, is_dir, mode, ..
        } = facts.runtime_dir
        else {
            panic!("the directory exists: {:?}", facts.runtime_dir);
        };
        assert_eq!(path, dir.path());
        assert!(is_dir);
        assert!(mode > 0, "a mode was read");

        let gone = env_with(&[
            ("PATH", "/does/not/exist"),
            ("XDG_RUNTIME_DIR", "/definitely/not/here"),
        ]);
        let (daemon, llm) = nothing();
        assert!(matches!(
            Probe::new(&gone).collect(daemon, llm).runtime_dir,
            RuntimeDirFacts::Missing { .. }
        ));

        let unset = env_with(&[("PATH", "/does/not/exist")]);
        let (daemon, llm) = nothing();
        let RuntimeDirFacts::Unset { expected } =
            Probe::new(&unset).collect(daemon, llm).runtime_dir
        else {
            panic!("without the variable there is nothing to look at");
        };
        assert_eq!(expected, PathBuf::from("/run/user/4242"));
    }

    #[test]
    fn the_tray_library_is_found_in_a_directory_the_loader_config_names() {
        let root = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(root.path().join("etc/ld.so.conf.d")).unwrap();
        std::fs::create_dir_all(root.path().join("opt/libs")).unwrap();
        std::fs::write(
            root.path().join("etc/ld.so.conf.d/local.conf"),
            "# a comment\n/opt/libs\n",
        )
        .unwrap();
        std::fs::write(
            root.path().join("opt/libs/libayatana-appindicator3.so.1"),
            "",
        )
        .unwrap();

        let env = env_with(&[("PATH", "/does/not/exist"), ("XDG_CURRENT_DESKTOP", "KDE")]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).with_root(root.path()).collect(daemon, llm);
        assert_eq!(
            facts.tray.library,
            Some(root.path().join("opt/libs/libayatana-appindicator3.so.1"))
        );
        assert!(facts.tray.readable_dirs > 0);
        assert_eq!(facts.tray.desktop.as_deref(), Some("KDE"));
    }

    #[test]
    fn a_root_without_any_library_directory_measures_nothing() {
        let root = tempfile::tempdir().unwrap();
        let env = env_with(&[("PATH", "/does/not/exist")]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).with_root(root.path()).collect(daemon, llm);
        assert_eq!(facts.tray.readable_dirs, 0);
        assert!(facts.tray.searched_dirs > 0, "it did look somewhere");
        assert_eq!(facts.tray.library, None);
    }

    #[test]
    fn the_free_space_is_measured_at_the_first_ancestor_that_exists() {
        let dir = tempfile::tempdir().unwrap();
        let env = env_with(&[
            ("PATH", "/does/not/exist"),
            (
                "XDG_DATA_HOME",
                &dir.path().join("not/created/yet").display().to_string(),
            ),
        ]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);
        let super::DiskFacts::Measured {
            path,
            available_bytes,
        } = facts.disk
        else {
            panic!("a temp dir has a file system: {:?}", facts.disk);
        };
        assert!(path.starts_with(dir.path()), "{}", path.display());
        assert!(available_bytes > 0);
    }

    #[test]
    fn bwrap_is_started_without_a_single_variable_of_the_host() {
        let dir = tempfile::tempdir().unwrap();
        script(
            dir.path(),
            "bwrap",
            "echo \"HOME=[${HOME:-}] RUNTIME=[${XDG_RUNTIME_DIR:-}]\"\necho 'bubblewrap 0.9.1'",
        );
        let env = env_with(&[
            ("PATH", &dir.path().display().to_string()),
            ("HOME", "/home/secret"),
            ("XDG_RUNTIME_DIR", "/run/user/4242"),
        ]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);
        let run = facts.userns.probe.found().expect("the probe ran");
        assert!(
            run.stdout.contains("HOME=[] RUNTIME=[]"),
            "bwrap must see nothing of the host: {:?}",
            run.stdout
        );
    }

    #[test]
    fn the_agent_and_systemctl_see_the_four_variables_of_their_own_session() {
        let dir = tempfile::tempdir().unwrap();
        script(
            dir.path(),
            "my-agent",
            "echo \"HOME=[${HOME:-}] BUS=[${DBUS_SESSION_BUS_ADDRESS:-}] \
             PRELOAD=[${LD_PRELOAD:-}]\"",
        );
        let env = env_with(&[
            ("PATH", &dir.path().display().to_string()),
            ("HOME", "/home/u"),
            ("DBUS_SESSION_BUS_ADDRESS", "unix:path=/run/user/4242/bus"),
            ("LD_PRELOAD", "/tmp/evil.so"),
        ]);
        let (daemon, llm) = nothing();
        let facts = probe(&env)
            .with_agent("opencode", Some("my-agent".to_owned()))
            .collect(daemon, llm);
        let super::AgentFacts::Found { version, .. } = facts.agent else {
            panic!("the fake agent must be found: {:?}", facts.agent);
        };
        let Reading::Found(line) = version else {
            panic!("the agent answered: {version:?}");
        };
        assert!(line.contains("HOME=[/home/u]"), "{line}");
        assert!(
            line.contains("BUS=[unix:path=/run/user/4242/bus]"),
            "{line}"
        );
        // Der Lader bleibt draussen, auch wo die Sitzung hineindarf.
        assert!(line.contains("PRELOAD=[]"), "{line}");
    }

    #[test]
    fn no_loader_variable_travels_into_a_call_of_the_doctor() {
        for key in humanitl_config::LOADER_ENV_KEYS {
            assert!(
                !super::SESSION_ENV_KEYS.contains(key),
                "{key} must not be handed to a program the doctor starts"
            );
        }
    }

    /// Unlesbare Bytes sind keine gelesene Fassung.
    ///
    /// `from_utf8_lossy` machte aus `\xffbubblewrap 0.9.1` eine gemessene
    /// Fassung und meldete `ok` ueber ein `bwrap`, dessen Ausgabe nicht lesbar
    /// war.
    #[test]
    fn output_that_is_not_utf8_is_unreadable_and_not_a_measurement() {
        let dir = tempfile::tempdir().unwrap();
        // Die Bytes stehen in einer Datei, nicht in einem `printf`: Wie viele
        // Oktalziffern eine Shell hinter `\0` liest, ist zwischen `dash` und
        // `bash` verschieden, und der Test soll ein Byte pruefen und nicht
        // eine Shell.
        let raw = dir.path().join("answer.bin");
        std::fs::write(&raw, b"\xffbubblewrap 0.9.1\n").unwrap();
        script(dir.path(), "bwrap", &format!("/bin/cat {}", raw.display()));
        let env = env_with(&[("PATH", &dir.path().display().to_string())]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);

        let BwrapFacts::Unreadable { error, .. } = facts.bwrap else {
            panic!("bytes that are not UTF-8 were not read: {:?}", facts.bwrap);
        };
        assert!(error.contains("not valid UTF-8"), "{error}");
        // Und die Namensraum-Probe desselben Programms ebenso.
        assert!(
            matches!(facts.userns.probe, Reading::Unreadable(_)),
            "{:?}",
            facts.userns.probe
        );
    }

    #[test]
    fn an_agent_whose_version_is_not_utf8_is_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let raw = dir.path().join("answer.bin");
        std::fs::write(&raw, b"\xff1.2.3\n").unwrap();
        script(
            dir.path(),
            "my-agent",
            &format!("/bin/cat {}", raw.display()),
        );
        let env = env_with(&[("PATH", &dir.path().display().to_string())]);
        let (daemon, llm) = nothing();
        let facts = probe(&env)
            .with_agent("opencode", Some("my-agent".to_owned()))
            .collect(daemon, llm);
        let super::AgentFacts::Found { version, .. } = facts.agent else {
            panic!("the fake agent must be found: {:?}", facts.agent);
        };
        let Reading::Unreadable(error) = version else {
            panic!("bytes that are not UTF-8 were not read: {version:?}");
        };
        assert!(error.contains("not valid UTF-8"), "{error}");
    }

    /// Eine Fassung, die nicht zu lesen ist, wird nicht stillschweigend null.
    ///
    /// `Version::parse` machte aus `bubblewrap 999999999999999999.9` ein
    /// `0.9.0`; das besteht die Mindestpruefung von `0.8.0`, und der Doctor
    /// bescheinigte eine Fassung, die er gar nicht kennt.
    #[test]
    fn a_version_number_that_does_not_fit_is_refused_and_not_rounded_to_zero() {
        assert_eq!(parse_version("bubblewrap 0.9.1"), Ok(Version(0, 9, 1)));
        assert_eq!(parse_version("0.8"), Ok(Version(0, 8, 0)));
        assert_eq!(parse_version("bubblewrap 1.2.3.4"), Ok(Version(1, 2, 3)));

        let overflow = parse_version("bubblewrap 999999999999999999.9")
            .expect_err("a number that does not fit is no version");
        assert!(overflow.contains("999999999999999999"), "{overflow}");

        let empty = parse_version("command not found").expect_err("no number is no version");
        assert!(empty.contains("carries no number"), "{empty}");

        // Und der ganze Weg: das gefundene bwrap gilt als unlesbar, nicht als
        // ausreichend.
        let dir = tempfile::tempdir().unwrap();
        script(
            dir.path(),
            "bwrap",
            "echo 'bubblewrap 999999999999999999.9'",
        );
        let env = env_with(&[("PATH", &dir.path().display().to_string())]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);
        assert!(
            matches!(facts.bwrap, BwrapFacts::Unreadable { .. }),
            "{:?}",
            facts.bwrap
        );
    }

    #[test]
    fn the_framebuffer_driver_is_not_the_nvidia_driver() {
        assert!(is_nvidia_module("nvidia"));
        assert!(is_nvidia_module("nvidia_drm"));
        assert!(is_nvidia_module("nvidia_modeset"));
        assert!(!is_nvidia_module("nvidiafb"));
        assert!(!is_nvidia_module("amdgpu"));
    }

    #[test]
    fn an_agent_command_from_the_configuration_is_the_one_that_is_searched() {
        let dir = tempfile::tempdir().unwrap();
        script(dir.path(), "my-agent", "echo '9.9.9'");
        let env = env_with(&[("PATH", &dir.path().display().to_string())]);
        let (daemon, llm) = nothing();
        let facts = probe(&env)
            .with_agent("opencode", Some("my-agent".to_owned()))
            .collect(daemon, llm);
        let super::AgentFacts::Found {
            command, version, ..
        } = facts.agent
        else {
            panic!("the configured command must be found: {:?}", facts.agent);
        };
        assert_eq!(command, "my-agent");
        assert_eq!(version, Reading::Found("9.9.9".to_owned()));
    }

    #[test]
    fn an_agent_that_is_not_installed_carries_the_command_that_installs_it() {
        let env = env_with(&[("PATH", "/does/not/exist")]);
        let (daemon, llm) = nothing();
        let facts = probe(&env).collect(daemon, llm);
        let super::AgentFacts::Missing {
            command, install, ..
        } = facts.agent
        else {
            panic!("nothing is installed here: {:?}", facts.agent);
        };
        assert_eq!(command, "opencode");
        assert!(install.contains("opencode.ai"), "{install}");
    }
}
