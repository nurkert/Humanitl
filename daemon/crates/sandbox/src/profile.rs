//! Das Sandbox-Profil: die Politik der Sandbox als lesbare Datei.
//!
//! Ein Profil ist die einzige Quelle dafür, was in der Sandbox liegt, was sie
//! erreicht und was der Agent in seiner Umgebung vorfindet (ADR-002). Aus ihm
//! entsteht deterministisch die bwrap-Kommandozeile
//! ([`SandboxProfile::to_bwrap_args`]), die die Oberfläche wörtlich anzeigt.
//! Zusammengeklebt wird nichts: was nicht im Profil steht, ist nicht in der
//! Sandbox.
//!
//! Zwei Profile gehören zum Auslieferungsumfang, `profiles/sandbox/default.toml`
//! und `profiles/sandbox/test.toml`. Nutzer legen eigene unter
//! `$XDG_CONFIG_HOME/humanitl/profiles/` ab; deshalb ist das Laden hier kein
//! reines Deserialisieren, sondern eine Prüfung.
//!
//! # Die Mount-Allowlist
//!
//! Ein Profil, das `/var/run/docker.sock`, `$XDG_RUNTIME_DIR` oder `~/.ssh`
//! einhängt, hebt die Isolation auf, ohne dass es jemandem auffällt. Deshalb
//! prüft [`SandboxProfile::validate_with`] jede Host-Quelle gegen eine fest
//! verdrahtete Denylist ([`FORBIDDEN_MOUNTS`], [`FORBIDDEN_IN_HOME`]). Die Liste
//! ist nicht konfigurierbar: eine Sicherheitsgrenze, die sich in derselben Datei
//! abschalten lässt, die sie schützt, ist keine.
//!
//! Verboten ist ein Pfad in drei Richtungen: der Eintrag selbst, alles darunter
//! (bei ganzen Bäumen) und alles darüber, denn wer `/` oder `/home` einhängt,
//! bekommt `~/.ssh` mit. Geprüft wird sowohl der geschriebene als auch der
//! aufgelöste Pfad, damit ein Symlink nichts einschmuggelt; dasselbe gilt für das
//! Heimatverzeichnis selbst, das auf manchen Systemen hinter `/var/home` liegt.
//!
//! Welche Verzeichnisse dieser Maschine geschützt sind, sagt die
//! [`MountPolicy`]: das Heimatverzeichnis, `$XDG_RUNTIME_DIR`,
//! `$XDG_CONFIG_HOME/humanitl` und `$XDG_DATA_HOME/humanitl`, die alle
//! außerhalb von `/run` und `$HOME` liegen können. Der Launcher (HUM-011) baut
//! sie mit [`MountPolicy::from_paths`] aus `humanitl_config::Paths`, nie aus
//! dem Heimatverzeichnis allein; [`SandboxProfile::load_validated`] nimmt sie
//! entgegen.
//!
//! Eine Quelle, die auf dem Host ein Unix-Socket ist, ist immer verboten, und
//! ebenso ein Verzeichnis, unter dem ein begrenzter Suchlauf einen findet
//! ([`SOCKET_WALK_MAX_DEPTH`], [`SOCKET_WALK_MAX_ENTRIES`], Symlinks werden
//! nicht verfolgt): der einzige Socket in der Sandbox ist der Proxy-Socket, und
//! der kommt aus dem [`SessionContext`], nicht aus dem Profil. Der Suchlauf ist
//! eine Prüfung mit Budget, kein Beweis; den führt der Isolation-Check in der
//! Sandbox (HUM-041).
//!
//! # Was ein Profil nicht abschwächen kann
//!
//! `sandbox.unshare` schreibt `--unshare-all` aus und darf nicht kürzer sein
//! ([`Namespace::ALL`]); `sandbox.die_with_parent` und `sandbox.new_session`
//! dürfen nicht `false` sein; `sandbox.hostname` ist [`HOSTNAME`];
//! `mounts.tmpfs` muss [`REQUIRED_TMPFS`] enthalten. `seccomp.deny_syscalls`
//! wird beim Lesen mit [`DEFAULT_DENY_SYSCALLS`] vereinigt und kann deshalb nur
//! wachsen; [`MANDATORY_MASKED_FILES`] werden beim Rendern mit
//! `mounts.masked_files` vereinigt, ein Profil kann sie ergänzen, nicht
//! streichen. Alles prüft [`SandboxProfile::parse`].
//!
//! Zwei Böden wirken in beide Richtungen, weil ein Profil sie sonst aufweichen
//! könnte, statt sie nur zu verschärfen:
//!
//! - `seccomp.allow_families` und `seccomp.allow_types` sind genau
//!   [`REQUIRED_SOCKET_FAMILIES`] und [`REQUIRED_SOCKET_TYPES`]. Ein Profil,
//!   das `AF_UNIX` oder `SOCK_DGRAM` nennt, wird mit `CONFIG_003` abgelehnt;
//!   die einzige Ausnahme ist [`SocketFloor::BrowserUnixIpc`], und die steht
//!   im Rust-Code des Launchers, nicht in einer Datei (dritte Garantie).
//! - `network.bridges` ist genau die Proxy-Bridge
//!   ([`Bridge::proxy_on`]). Eine zweite Bridge wäre eine zweite Tür, auch
//!   mit `dir = "in"`, denn der Shim öffnet jede, die er bekommt (zweite
//!   Garantie).
//!
//! # Fehlercodes
//!
//! - `CONFIG_001` — Datei fehlt oder ist kein TOML.
//! - `CONFIG_002` — ein Schlüssel oder Abschnitt ist unbekannt.
//! - `CONFIG_003` — ein Wert ist unzulässig oder widerspricht einem anderen.
//! - `SANDBOX_006` — eine Host-Quelle steht auf der Denylist.
//! - `SANDBOX_007` — eine Bridge zeigt nach außen; das kann der Shim noch nicht.

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsString;
use std::net::SocketAddr;
use std::os::unix::fs::FileTypeExt;
use std::path::{Component, Path, PathBuf};

use humanitl_config::{Env, Paths, WorkMode};
use humanitl_core::diagnostics::codes::{
    CONFIG_001, CONFIG_002, CONFIG_003, SANDBOX_006, SANDBOX_007,
};
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

/// Das Projektverzeichnis in der Sandbox.
pub const WORK_DST: &str = "/work";

/// Der Proxy-Socket in der Sandbox.
pub const PROXY_SOCKET_DST: &str = "/run/humanitl/proxy.sock";

/// Das Zertifikat der eigenen CA in der Sandbox.
pub const CA_CERT_DST: &str = "/etc/humanitl/ca.crt";

/// Der System-Vertrauensspeicher in der Sandbox, den der Launcher überdeckt.
///
/// Die Sandbox sieht unter diesem Pfad nicht das Bundle des Hosts, sondern das
/// erzeugte aus [`SessionContext::ca_bundle_src`]: die Wurzeln des Systems
/// plus die eigene CA (HUM-014). Ohne diese Überdeckung lehnt jeder
/// TLS-Client in der Sandbox das Leaf des Proxys ab, und die Sandbox hätte
/// zwar eine Tür, aber keine, durch die etwas käme.
///
/// Nicht im Profil einstellbar: es ist der Pfad, den OpenSSL, `GnuTLS` und die
/// Werkzeuge darüber lesen, und ein Profil, das ihn verschieben könnte, würde
/// die Überdeckung unbemerkt abschalten. Der Bind kommt nach dem `--ro-bind`
/// von `/etc/ssl` aus `mounts.ro`, sonst verdeckte ihn dieses wieder; der
/// Mountpoint muss dort schon liegen, weil `bwrap` ihn in einem nur lesbaren
/// Bind nicht anlegen kann (ein Host ohne `ca-certificates` hat ihn nicht).
pub const CA_BUNDLE_DST: &str = "/etc/ssl/certs/ca-certificates.crt";

/// Der Shim in der Sandbox.
///
/// Unter `/run/humanitl`, neben dem Proxy-Socket, und nicht unter `/usr`:
/// `bwrap` legt den Mountpoint einer Datei erst beim Einhängen an, und in
/// einem nur lesbaren Bind von `/usr` scheitert das mit `EROFS`. `/run` steht
/// auf der Denylist, also ist `/run/humanitl` in jeder Sandbox das eigene
/// tmpfs von `bwrap` und nie ein Host-Verzeichnis (HUM-011).
pub const SHIM_DST: &str = "/run/humanitl/humanitl-shim";

/// Der Port, auf dem die Bridge in der Sandbox lauscht.
pub const PROXY_PORT: u16 = 3128;

/// Der Name der Bridge, die den Proxy erreichbar macht.
pub const PROXY_BRIDGE: &str = "proxy";

/// Die einzige Fassung des Profilformats.
pub const PROFILE_VERSION: u32 = 1;

/// Der einzige zulässige Hostname in der Sandbox.
///
/// Ein Profil, das etwas anderes nennt, wird mit `CONFIG_003` abgelehnt: der
/// Name erscheint in Prompts und Logs des Agenten und soll überall dasselbe
/// sagen, nämlich dass dies keine Maschine des Nutzers ist.
pub const HOSTNAME: &str = "sandbox";

/// Dateien unter `/work`, die in jeder Sandbox überdeckt werden.
///
/// `.envrc` führt `direnv` beim Betreten aus, `.git/config` trägt
/// Credential-Helper und Hooks-Pfade. Beide werden beim Rendern mit
/// `mounts.masked_files` vereinigt ([`SandboxProfile::effective_masked_files`]):
/// ein Profil kann weitere Dateien überdecken, diese beiden aber nicht
/// freigeben, auch nicht mit `masked_files = []`.
pub const MANDATORY_MASKED_FILES: &[&str] = &["/work/.envrc", "/work/.git/config"];

/// Ziele, die `mounts.tmpfs` in jedem Profil nennen muss.
///
/// Ohne ein tmpfs auf `/tmp` sähe die Sandbox das Host-`/tmp` samt
/// `.X11-unix`; ohne eines auf `/dev/shm` teilte sie den POSIX-Shared-Memory
/// des Hosts. Fehlt eines, lehnt [`SandboxProfile::parse`] mit `CONFIG_003` ab.
pub const REQUIRED_TMPFS: &[&str] = &["/tmp", "/dev/shm"];

/// Wie tief der Socket-Suchlauf unter einer Verzeichnisquelle geht.
///
/// Einträge bis zu dieser Tiefe werden angesehen (`quelle/a/b/c` liegt in
/// Tiefe 3); tiefer wird nicht gelesen. Siehe [`MountPolicy::check`].
pub const SOCKET_WALK_MAX_DEPTH: usize = 3;

/// Wie viele Einträge der Socket-Suchlauf höchstens ansieht.
///
/// Ein Bind von `/usr` darf die Prüfung nicht sekundenlang aufhalten; ist das
/// Budget aufgebraucht, endet der Suchlauf ohne Befund. Siehe
/// [`MountPolicy::check`].
pub const SOCKET_WALK_MAX_ENTRIES: usize = 2000;

/// Die Socket-Familien, die `socket(2)` in der Sandbox öffnen darf, in der
/// Reihenfolge, in der der Launcher sie an den Shim reicht.
///
/// Der Boden wirkt in beide Richtungen: genau diese beiden, nicht weniger und
/// nicht mehr. `AF_INET` und `AF_INET6` sind nötig, weil der Agent den Proxy
/// über `127.0.0.1:3128` erreicht; alles andere, `AF_UNIX` voran, wäre eine
/// zweite Tür, die der leere Netz-Namensraum nicht mehr auffangen kann
/// (`docs/SECURITY.md` Satz 3). Die einzige Ausnahme ist
/// [`SocketFloor::BrowserUnixIpc`].
pub const REQUIRED_SOCKET_FAMILIES: &[SocketFamily] =
    &[SocketFamily::AfInet, SocketFamily::AfInet6];

/// Der einzige Socket-Typ, den `socket(2)` in der Sandbox öffnen darf.
///
/// Auch dieser Boden wirkt in beide Richtungen. `SOCK_DGRAM` und `SOCK_RAW`
/// bleiben in jedem Profil gesperrt, auch im späteren `browser`: UDP wäre der
/// Weg an der Aufzeichnung vorbei (DNS, QUIC), und dafür gibt es keinen
/// Anwendungsfall, der die Garantie aufwöge.
pub const REQUIRED_SOCKET_TYPES: &[SocketType] = &[SocketType::SockStream];

/// Syscalls, die in keinem Profil erlaubt sind (`CONVENTIONS.md` 4.8).
///
/// `ptrace` und `process_vm_*` lesen fremde Prozesse aus, `io_uring_*` führt
/// Ein- und Ausgabe an seccomp vorbei, die `key`-Aufrufe erreichen den
/// Schlüsselbund des Kernels. Danach folgt die Standard-Härtung aus der
/// Tabelle von HUM-012 (`backlog/sprint-1.md`), dieselbe, die das
/// Docker-Standardprofil verbietet: einen neuen Kern laden (`kexec_*`),
/// Kernmodule tauschen (`*_module`), BPF-Programme laden (`bpf`), fremde
/// Ereigniszähler öffnen (`perf_event_open`) und Seitenfehler im eigenen
/// Adressraum selbst bedienen (`userfaultfd`).
///
/// Die Liste ist der Boden und deckt sich Zeile für Zeile mit `FLOOR` in
/// `daemon/bin/humanitl-shim/src/seccomp.rs`: [`SandboxProfile::parse`]
/// vereinigt sie mit `seccomp.deny_syscalls` des Profils, die Liste hier zuerst,
/// dann die Ergänzungen des Profils, ohne Doppelungen. Ein Profil, das nur
/// `["mount"]` schreibt, verbietet damit achtzehn Syscalls, nicht einen.
pub const DEFAULT_DENY_SYSCALLS: &[&str] = &[
    "ptrace",
    "io_uring_setup",
    "io_uring_enter",
    "io_uring_register",
    "process_vm_readv",
    "process_vm_writev",
    "keyctl",
    "add_key",
    "request_key",
    "kexec_load",
    "kexec_file_load",
    "init_module",
    "finit_module",
    "delete_module",
    "bpf",
    "perf_event_open",
    "userfaultfd",
];

/// Ein ganzes Profil, so wie es in der TOML-Datei steht.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SandboxProfile {
    /// Fassung des Formats, derzeit immer [`PROFILE_VERSION`].
    pub version: u32,
    /// Name des Profils, wie ihn `--profile` nennt.
    pub name: String,
    /// Ein Satz für die Oberfläche.
    #[serde(default)]
    pub description: String,
    /// Backend, Namensräume, Lebensdauer.
    #[serde(default)]
    pub sandbox: SandboxSection,
    /// Was eingehängt, überdeckt und verlinkt wird.
    #[serde(default)]
    pub mounts: MountSection,
    /// Der einzige Weg nach draußen.
    #[serde(default)]
    pub network: NetworkSection,
    /// Was der Shim nach dem Start der Bridges noch erlaubt.
    #[serde(default)]
    pub seccomp: SeccompSection,
    /// Die vollständige Umgebung des Agenten. Vom Host wird nichts geerbt.
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

/// Backend, Namensräume und Lebensdauer der Sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SandboxSection {
    /// Der Name des Backends; im MVP nur `bwrap`.
    pub backend: String,
    /// Der Hostname in der Sandbox. Muss [`HOSTNAME`] sein.
    pub hostname: String,
    /// Die Namensräume, die abgetrennt werden. Muss alle aus [`Namespace::ALL`]
    /// nennen; die Reihenfolge in der Kommandozeile ist die von `ALL`, nicht
    /// die des Profils.
    pub unshare: Vec<Namespace>,
    /// Beendet die Sandbox, wenn der Daemon endet. Darf nicht `false` sein;
    /// `--die-with-parent` steht in jeder Kommandozeile.
    pub die_with_parent: bool,
    /// Eigene Session, damit kein `TIOCSTI` in das Terminal des Nutzers
    /// zurückschreibt. Darf nicht `false` sein; `--new-session` steht in jeder
    /// Kommandozeile.
    pub new_session: bool,
    /// Die kleinste `bwrap`-Fassung, mit der dieses Profil läuft. Geprüft in HUM-011.
    pub min_bwrap_version: String,
}

impl Default for SandboxSection {
    fn default() -> Self {
        Self {
            backend: "bwrap".to_owned(),
            hostname: HOSTNAME.to_owned(),
            unshare: Namespace::ALL.to_vec(),
            die_with_parent: true,
            new_session: true,
            min_bwrap_version: "0.8.0".to_owned(),
        }
    }
}

/// Ein Linux-Namensraum, den die Sandbox abtrennt.
///
/// `sandbox.unshare` ist keine Auswahl, sondern `--unshare-all` ausgeschrieben,
/// damit die Zeile lesbar bleibt (HUM-010). Ein Profil, das einen Namensraum
/// weglässt, wird abgelehnt; siehe [`Namespace::ALL`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Namespace {
    /// Nutzer und Gruppen.
    User,
    /// Prozesstabelle.
    Pid,
    /// Netzwerk. Ohne diesen Namensraum gibt es die erste Garantie nicht.
    Net,
    /// System-V-IPC und POSIX-Message-Queues.
    Ipc,
    /// Hostname und Domainname.
    Uts,
    /// Control-Groups.
    Cgroup,
}

impl Namespace {
    /// Alle Namensräume, die `--unshare-all` abtrennt, in der Reihenfolge, in
    /// der die Kommandozeile sie nennt. Jedes Profil muss sie alle nennen; wie
    /// es sie ordnet, ist für die Zeile ohne Belang.
    pub const ALL: [Self; 6] = [
        Self::User,
        Self::Pid,
        Self::Net,
        Self::Ipc,
        Self::Uts,
        Self::Cgroup,
    ];

    /// Warum die Sandbox diesen Namensraum nicht behalten darf; steht im
    /// Befund, wenn ein Profil ihn weglässt.
    #[must_use]
    pub const fn why_required(self) -> &'static str {
        match self {
            Self::User => "without it an unprivileged bwrap cannot mount anything",
            Self::Pid => {
                "without it the fresh /proc shows the host's processes and their environment"
            }
            Self::Net => "without it the sandbox keeps the host's network interfaces",
            Self::Ipc => {
                "without it the sandbox shares System V memory and message queues with the host"
            }
            Self::Uts => "without it bwrap refuses --hostname",
            Self::Cgroup => "without it /proc/self/cgroup names the host's control groups",
        }
    }

    /// Das Argument, das `bwrap` dafür erwartet.
    #[must_use]
    pub const fn flag(self) -> &'static str {
        match self {
            Self::User => "--unshare-user",
            Self::Pid => "--unshare-pid",
            Self::Net => "--unshare-net",
            Self::Ipc => "--unshare-ipc",
            Self::Uts => "--unshare-uts",
            Self::Cgroup => "--unshare-cgroup",
        }
    }

    /// Der Name, wie er im Profil steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Pid => "pid",
            Self::Net => "net",
            Self::Ipc => "ipc",
            Self::Uts => "uts",
            Self::Cgroup => "cgroup",
        }
    }
}

/// Was eingehängt, überdeckt und verlinkt wird.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MountSection {
    /// Das Projektverzeichnis. Die Quelle kommt aus der Sitzung.
    pub work: WorkMount,
    /// Nur lesbare Einhängungen mit gleicher Quelle und gleichem Ziel.
    pub ro: Vec<PathBuf>,
    /// Symbolische Verweise, die `bwrap` in der Sandbox anlegt.
    pub symlinks: Vec<Symlink>,
    /// Leere Dateisysteme im Arbeitsspeicher. Muss [`REQUIRED_TMPFS`] enthalten.
    pub tmpfs: Vec<PathBuf>,
    /// Wohin `/proc` eingehängt wird.
    pub proc: Option<PathBuf>,
    /// Wohin das minimale `/dev` eingehängt wird.
    pub dev: Option<PathBuf>,
    /// Dateien, die von einer leeren, nur lesbaren Datei überdeckt werden,
    /// zusätzlich zu [`MANDATORY_MASKED_FILES`].
    pub masked_files: Vec<PathBuf>,
    /// Zusätzliche nur lesbare Einhängungen des Nutzers.
    pub extra_ro: Vec<PathBuf>,
    /// Zusätzliche beschreibbare Einhängungen des Nutzers.
    pub extra_rw: Vec<PathBuf>,
}

impl Default for MountSection {
    fn default() -> Self {
        Self {
            work: WorkMount::default(),
            ro: Vec::new(),
            symlinks: Vec::new(),
            tmpfs: REQUIRED_TMPFS.iter().map(PathBuf::from).collect(),
            proc: Some(PathBuf::from("/proc")),
            dev: Some(PathBuf::from("/dev")),
            masked_files: Vec::new(),
            extra_ro: Vec::new(),
            extra_rw: Vec::new(),
        }
    }
}

/// Das Projektverzeichnis in der Sandbox.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WorkMount {
    /// Der Ort in der Sandbox, üblicherweise [`WORK_DST`].
    pub dst: PathBuf,
    /// Der Modus des Profils. Fehlt er, entscheidet allein die Sitzung.
    pub mode: Option<WorkMode>,
}

impl Default for WorkMount {
    fn default() -> Self {
        Self {
            dst: PathBuf::from(WORK_DST),
            mode: None,
        }
    }
}

/// Ein symbolischer Verweis, den `bwrap` in der Sandbox anlegt.
///
/// Steht im Profil als Paar, `["usr/lib", "/lib"]`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "(String, PathBuf)")]
pub struct Symlink {
    /// Das Ziel des Verweises, so wie es in der Sandbox gilt.
    pub target: String,
    /// Der Ort des Verweises in der Sandbox.
    pub link: PathBuf,
}

impl From<(String, PathBuf)> for Symlink {
    fn from((target, link): (String, PathBuf)) -> Self {
        Self { target, link }
    }
}

/// Der einzige Weg aus der Sandbox heraus.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NetworkSection {
    /// Der Proxy-Socket in der Sandbox.
    pub proxy_socket_dst: PathBuf,
    /// Der Port, auf dem die Bridge in der Sandbox lauscht.
    pub proxy_port: u16,
    /// Das CA-Zertifikat in der Sandbox.
    pub ca_cert_dst: PathBuf,
    /// Der Shim in der Sandbox.
    pub shim_dst: PathBuf,
    /// Die Bridges, die der Shim vor seccomp aufmacht.
    pub bridges: Vec<Bridge>,
}

impl Default for NetworkSection {
    fn default() -> Self {
        Self {
            proxy_socket_dst: PathBuf::from(PROXY_SOCKET_DST),
            proxy_port: PROXY_PORT,
            ca_cert_dst: PathBuf::from(CA_CERT_DST),
            shim_dst: PathBuf::from(SHIM_DST),
            bridges: vec![Bridge::proxy()],
        }
    }
}

/// Eine Brücke zwischen einem TCP-Port in der Sandbox und einem Unix-Socket.
///
/// Serialisiert in genau dieser Feldreihenfolge nach JSON, weil der Launcher
/// die Liste so an den Shim reicht ([`crate::bridge_env::ENV_BRIDGES`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bridge {
    /// Der Name, unter dem Oberfläche und Protokoll sie nennen.
    pub name: String,
    /// Wer verbindet und wer lauscht.
    pub dir: BridgeDirection,
    /// Die Adresse in der Sandbox.
    pub listen: SocketAddr,
    /// Der Unix-Socket, den die Bridge bedient.
    pub socket: PathBuf,
}

impl Bridge {
    /// Die Bridge des MVP: Sandbox-TCP 127.0.0.1:3128 auf den Proxy-Socket.
    #[must_use]
    pub fn proxy() -> Self {
        Self::proxy_on(PROXY_PORT)
    }

    /// Dieselbe Bridge auf einem anderen Port.
    ///
    /// Die einzige Bridge, die ein Profil deklarieren darf: Name
    /// [`PROXY_BRIDGE`], Richtung `in`, Adresse `127.0.0.1` mit
    /// `network.proxy_port`, Ziel [`PROXY_SOCKET_DST`].
    /// [`SandboxProfile::parse`] vergleicht `network.bridges` damit und lehnt
    /// jede Abweichung ab: jede weitere Bridge wäre eine zweite Tür, weil der
    /// Shim öffnet, was in der Liste steht (zweite Garantie,
    /// `docs/SECURITY.md` Satz 2).
    #[must_use]
    pub fn proxy_on(port: u16) -> Self {
        Self {
            name: PROXY_BRIDGE.to_owned(),
            dir: BridgeDirection::In,
            listen: SocketAddr::from(([127, 0, 0, 1], port)),
            socket: PathBuf::from(PROXY_SOCKET_DST),
        }
    }
}

/// Die Richtung einer Bridge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BridgeDirection {
    /// Die Sandbox verbindet sich nach draußen: TCP in der Sandbox, Unix-Socket auf dem Host.
    In,
    /// Der Host verbindet sich hinein: Unix-Socket in der Sandbox, TCP in der Sandbox.
    ///
    /// Gedacht für den Debug-Kanal des Browsers. Der Shim kann das noch nicht;
    /// ein Profil, das es verlangt, wird mit `SANDBOX_007` abgelehnt.
    Out,
}

impl BridgeDirection {
    /// Der Name, wie er im Profil steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::In => "in",
            Self::Out => "out",
        }
    }
}

/// Was nach dem Start der Bridges noch erlaubt ist.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeccompSection {
    /// Erlaubte Socket-Familien für `socket(2)`. Muss genau
    /// [`REQUIRED_SOCKET_FAMILIES`] sein; nach dem Lesen steht hier immer
    /// dieser Boden in kanonischer Reihenfolge. `socketpair(2)` bleibt vom
    /// Filter unberührt: es kennt nur `AF_UNIX` und ist kein Egress
    /// (CONVENTIONS.md 4.11).
    pub allow_families: Vec<SocketFamily>,
    /// Erlaubte Socket-Typen; alles andere ist `EPERM`. Muss genau
    /// [`REQUIRED_SOCKET_TYPES`] sein.
    pub allow_types: Vec<SocketType>,
    /// Syscalls, die immer `EPERM` liefern. Nach dem Lesen immer
    /// [`DEFAULT_DENY_SYSCALLS`] zuerst, dann die Ergänzungen des Profils.
    pub deny_syscalls: Vec<String>,
}

impl Default for SeccompSection {
    fn default() -> Self {
        Self {
            allow_families: REQUIRED_SOCKET_FAMILIES.to_vec(),
            allow_types: REQUIRED_SOCKET_TYPES.to_vec(),
            deny_syscalls: DEFAULT_DENY_SYSCALLS
                .iter()
                .map(|name| (*name).to_owned())
                .collect(),
        }
    }
}

/// Eine Socket-Familie, wie sie `socket(2)` als erstes Argument bekommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SocketFamily {
    /// IPv4. Nötig für den Loopback zur Bridge.
    AfInet,
    /// IPv6. Nötig für den Loopback zur Bridge.
    AfInet6,
    /// Unix-Sockets. In keinem Profil erlaubt: der Wert existiert, damit ein
    /// Profil, das ihn nennt, einen Befund mit seinem Namen bekommt und nicht
    /// nur einen Parser-Fehler. Die einzige Stelle, die ihn zulässt, ist
    /// [`SocketFloor::BrowserUnixIpc`] im Code des Launchers (M7,
    /// Chromium-IPC).
    AfUnix,
}

impl SocketFamily {
    /// Der Name, wie er im Profil und in `SECURITY.md` steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AfInet => "AF_INET",
            Self::AfInet6 => "AF_INET6",
            Self::AfUnix => "AF_UNIX",
        }
    }
}

/// Ein Socket-Typ, wie ihn `socket(2)` als zweites Argument bekommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SocketType {
    /// Strom, also TCP oder ein Unix-Stream-Socket.
    SockStream,
    /// Datagramm. In keinem Profil erlaubt, auch im späteren `browser` nicht:
    /// UDP führte an der Aufzeichnung vorbei (DNS, QUIC). Der Wert existiert
    /// nur, damit ein Profil, das ihn nennt, einen Befund mit seinem Namen
    /// bekommt.
    SockDgram,
}

impl SocketType {
    /// Der Name, wie er im Profil und in `SECURITY.md` steht.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SockStream => "SOCK_STREAM",
            Self::SockDgram => "SOCK_DGRAM",
        }
    }
}

/// Der Boden der Socket-Politik, und die eine Ausnahme davon.
///
/// Der Filter des Shims ist die dritte Garantie. Ein Profil kann ihn
/// verschärfen (`seccomp.deny_syscalls` wächst), aber nicht aufweichen: welche
/// Familien und Typen `socket(2)` öffnen darf, steht in
/// [`REQUIRED_SOCKET_FAMILIES`] und [`REQUIRED_SOCKET_TYPES`] und nicht in der
/// Datei.
///
/// [`SocketFloor::BrowserUnixIpc`] ist die einzige vorgesehene Ausnahme, für
/// das Profil `browser` aus M7, das Chromium seine IPC über `AF_UNIX` führen
/// lassen muss (`docs/SECURITY.md`). Sie ist bewusst kein Schlüssel im Profil,
/// sondern ein Argument von [`SandboxProfile::parse_with_floor`]: ein Profil
/// liegt unter Umständen in einem geklonten Repository, und eine Grenze, die
/// die bewachte Datei selbst verschieben kann, ist keine. Voreingestellt ist
/// [`SocketFloor::Strict`]; [`SandboxProfile::load_validated`], der Einstieg
/// des Launchers, kennt nichts anderes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SocketFloor {
    /// Genau [`REQUIRED_SOCKET_FAMILIES`] mal [`REQUIRED_SOCKET_TYPES`].
    #[default]
    Strict,
    /// Zusätzlich `AF_UNIX`, sonst wie [`SocketFloor::Strict`]. Noch von
    /// keinem ausgelieferten Profil benutzt; reserviert für `browser` (M7).
    BrowserUnixIpc,
}

impl SocketFloor {
    /// Die Familien, die dieser Boden erlaubt, in kanonischer Reihenfolge.
    #[must_use]
    pub fn families(self) -> Vec<SocketFamily> {
        let mut families = REQUIRED_SOCKET_FAMILIES.to_vec();
        if self == Self::BrowserUnixIpc {
            families.push(SocketFamily::AfUnix);
        }
        families
    }

    /// Die Typen, die dieser Boden erlaubt. Für jeden Boden dieselben.
    #[must_use]
    pub fn types(self) -> Vec<SocketType> {
        REQUIRED_SOCKET_TYPES.to_vec()
    }
}

/// Alles, was erst zur Laufzeit feststeht.
///
/// Das Profil sagt, *wohin* etwas gehört; der Kontext sagt, *woher* es kommt.
/// Damit bleibt das Profil frei von Pfaden dieser Maschine und die erzeugte
/// Kommandozeile bleibt reproduzierbar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionContext {
    /// Die Sitzung, zu der diese Sandbox gehört.
    pub session: SessionId,
    /// Das Projektverzeichnis auf dem Host.
    pub work_src: PathBuf,
    /// Der Modus aus `sandbox.work_mode`.
    pub work_mode: WorkMode,
    /// Der Proxy-Socket auf dem Host.
    pub proxy_socket_src: PathBuf,
    /// Das CA-Zertifikat auf dem Host.
    pub ca_cert_src: PathBuf,
    /// Das erzeugte Bundle auf dem Host: die Wurzeln des Systems plus die
    /// eigene CA (`humanitl_proxy::ca::CaStore::bundle_path`). Der Launcher
    /// hängt es über [`CA_BUNDLE_DST`]; den Pfad reicht der Aufrufer herein,
    /// genau wie [`SessionContext::ca_cert_src`], damit diese Crate nichts
    /// über den Proxy wissen muss.
    pub ca_bundle_src: PathBuf,
    /// Der Shim auf dem Host.
    pub shim_src: PathBuf,
    /// Die Variablen der Sitzung, üblicherweise `humanitl_proxy::ca::env_kit`:
    /// das Env-Kit samt `HUMANITL_SESSION`. Sie überschreiben gleichnamige
    /// Einträge aus dem `[env]` des Profils und werden ihrerseits von den
    /// Variablen des Shims überschrieben
    /// ([`SandboxProfile::effective_env`]).
    pub session_env: Vec<(String, String)>,
    /// Der Befehl, den der Shim nach seccomp startet.
    pub command: Vec<OsString>,
}

/// Ein Eintrag der Denylist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MountRule {
    /// Der verbotene Pfad.
    pub path: &'static str,
    /// `true`: auch alles darunter. `false`: nur der Pfad selbst.
    ///
    /// Alles darüber ist in beiden Fällen verboten: ein Bind von `/` oder
    /// `/var` brächte den Eintrag mit.
    pub whole_tree: bool,
    /// Warum er verboten ist; steht im `why` des Befunds.
    pub reason: &'static str,
}

/// Host-Quellen, die in keiner Sandbox etwas zu suchen haben.
///
/// Nicht konfigurierbar (`backlog/sprint-0.md`, HUM-010). Der Proxy-Socket, das
/// CA-Zertifikat und der Shim kommen nicht von hier, sondern aus dem
/// [`SessionContext`]; sie sind die einzigen Ausnahmen und stehen deshalb nicht
/// im Profil.
pub static FORBIDDEN_MOUNTS: &[MountRule] = &[
    MountRule {
        path: "/proc",
        whole_tree: true,
        reason: "the kernel process table; the profile mounts a fresh /proc instead",
    },
    MountRule {
        path: "/sys",
        whole_tree: true,
        reason: "kernel objects, including the host network devices",
    },
    MountRule {
        path: "/dev",
        whole_tree: true,
        reason: "host device nodes; the profile mounts a minimal /dev instead",
    },
    MountRule {
        path: "/run",
        whole_tree: true,
        reason: "host runtime state: XDG_RUNTIME_DIR, the D-Bus session bus, docker.sock, the Wayland socket",
    },
    MountRule {
        path: "/var/run",
        whole_tree: true,
        reason: "host runtime state, usually the same tree as /run",
    },
    MountRule {
        path: "/tmp",
        whole_tree: false,
        reason: "the host /tmp as a whole; the profile mounts a tmpfs on /tmp instead",
    },
    MountRule {
        path: "/tmp/.X11-unix",
        whole_tree: true,
        reason: "the X11 socket directory; an X11 client can read every keystroke",
    },
    MountRule {
        path: "/var/tmp",
        whole_tree: false,
        reason: "the host /var/tmp as a whole; the profile mounts a tmpfs there instead",
    },
    MountRule {
        path: "/root",
        whole_tree: true,
        reason: "the superuser's home directory",
    },
];

/// Verbotene Pfade unterhalb des Heimatverzeichnisses, mit ihrem Grund.
///
/// Alles andere unterhalb des Heimatverzeichnisses ist ebenfalls verboten; diese
/// Liste dient nur der besseren Begründung. Das Projektverzeichnis kommt aus dem
/// [`SessionContext`] und wird nicht über das Profil eingehängt.
pub static FORBIDDEN_IN_HOME: &[(&str, &str)] = &[
    (".ssh", "private keys and known_hosts"),
    (".gnupg", "the GnuPG keyring"),
    (".gitconfig", "the git identity and its credential helper"),
    (".netrc", "credentials in plain text"),
    (".config/humanitl", "Humanitl's own configuration and rules"),
    (
        ".local/share/humanitl",
        "Humanitl's own database, blobs, audit log and CA key",
    ),
];

/// Die Umgebung, gegen die Host-Quellen geprüft werden.
///
/// Das Heimatverzeichnis steht nicht fest, und `$XDG_RUNTIME_DIR`,
/// `$XDG_CONFIG_HOME` oder `$XDG_DATA_HOME` müssen nicht darunter liegen. Alles
/// wird übergeben, nicht aus der Umgebung des Prozesses gelesen: eine Prüfung,
/// die von globalem Zustand abhängt, lässt sich nicht tabellengetrieben testen.
///
/// Jeder übergebene Pfad wird in beiden Schreibweisen geführt, wie geschrieben
/// und aufgelöst, damit ein Heimatverzeichnis hinter `/var/home` genauso
/// geschützt ist wie eines unter `/home`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountPolicy {
    /// Das Heimatverzeichnis in seinen Schreibweisen; leer, wenn es keine
    /// brauchbare Vergleichsbasis ist.
    homes: Vec<PathBuf>,
    /// Weitere ganze Bäume, die verboten sind, mit ihrem Grund.
    protected: Vec<(PathBuf, String)>,
}

/// Der Grund, mit dem [`MountPolicy::with_runtime_dir`] den Pfad schützt.
const RUNTIME_DIR_REASON: &str = "the runtime directory: it holds the daemon socket, the session token, the D-Bus session bus and the Wayland socket";

/// Der Grund, mit dem [`MountPolicy::with_config_dir`] den Pfad schützt.
const CONFIG_DIR_REASON: &str =
    "Humanitl's own configuration directory: config.toml, rules.yaml and the profiles";

/// Der Grund, mit dem [`MountPolicy::with_data_dir`] den Pfad schützt.
const DATA_DIR_REASON: &str =
    "Humanitl's own data directory: the database, the blobs, the audit log and the CA key";

/// Ob eine Quelle aus dem Profil oder das Projektverzeichnis geprüft wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    /// Eine Quelle aus dem Profil: nichts unter dem Heimatverzeichnis.
    Profile,
    /// Das Projektverzeichnis der Sitzung: darf unter dem Heimatverzeichnis liegen.
    WorkDir,
}

impl MountPolicy {
    /// Die Politik für dieses Heimatverzeichnis allein.
    ///
    /// Das genügt für Anzeige und Tests, nicht für den Start: `$XDG_RUNTIME_DIR`,
    /// `$XDG_CONFIG_HOME/humanitl` und `$XDG_DATA_HOME/humanitl` können
    /// außerhalb von `/run` und `$HOME` liegen und sind hier ungeschützt. Wer
    /// startet, nimmt [`MountPolicy::from_paths`] oder [`MountPolicy::from_dirs`].
    #[must_use]
    pub fn new(home: impl Into<PathBuf>) -> Self {
        let home = home.into();
        let homes = if is_meaningful_base(&home) {
            spellings(&home)
        } else {
            Vec::new()
        };
        Self {
            homes,
            protected: Vec::new(),
        }
    }

    /// Die vollständige Politik aus den vier geschützten Verzeichnissen.
    ///
    /// `runtime_dir` ist das ganze `$XDG_RUNTIME_DIR` (oder sein Ersatz),
    /// `config_dir` ist `$XDG_CONFIG_HOME/humanitl`, `data_dir` ist
    /// `$XDG_DATA_HOME/humanitl`. Jedes wird als ganzer Baum verboten, samt
    /// allem darüber. Wer die Werte nicht selbst hat, nimmt
    /// [`MountPolicy::from_paths`].
    #[must_use]
    pub fn from_dirs(
        home: impl Into<PathBuf>,
        runtime_dir: impl Into<PathBuf>,
        config_dir: impl Into<PathBuf>,
        data_dir: impl Into<PathBuf>,
    ) -> Self {
        Self::new(home)
            .with_runtime_dir(runtime_dir)
            .with_config_dir(config_dir)
            .with_data_dir(data_dir)
    }

    /// Die vollständige Politik aus den Pfaden der Konfiguration.
    ///
    /// Das ist der Einstieg für den Launcher (HUM-011): `Paths` kennt
    /// `$HOME`, `$XDG_RUNTIME_DIR` samt Ersatz unter `/run/user` oder `$TMPDIR`,
    /// `$XDG_CONFIG_HOME/humanitl` und `$XDG_DATA_HOME/humanitl`, und liest
    /// dafür nur die [`Env`], die ihm übergeben wurde. Geschützt wird das ganze
    /// `$XDG_RUNTIME_DIR`, wenn es gesetzt ist, und in jedem Fall das
    /// Verzeichnis, in dem Socket und Token liegen.
    #[must_use]
    pub fn from_paths(paths: &Paths) -> Self {
        let mut policy = Self::from_dirs(
            paths.home(),
            paths.runtime_dir().path,
            paths.config_dir(),
            paths.data_dir(),
        );
        if let Some(root) = paths.env().non_empty("XDG_RUNTIME_DIR") {
            policy = policy.with_runtime_dir(root);
        }
        policy
    }

    /// Die vollständige Politik aus einer übergebenen Umgebung.
    ///
    /// Liest `HOME`, `XDG_RUNTIME_DIR`, `XDG_CONFIG_HOME` und `XDG_DATA_HOME`
    /// aus `env`, nie aus der Umgebung des Prozesses; so bleibt die Prüfung
    /// tabellengetrieben testbar. Gleichbedeutend mit
    /// [`MountPolicy::from_paths`] über `Paths::new(env)`.
    #[must_use]
    pub fn from_env(env: &Env) -> Self {
        Self::from_paths(&Paths::new(env.clone()))
    }

    /// Ergänzt ein `$XDG_RUNTIME_DIR`, das nicht unter `/run/user` liegt.
    #[must_use]
    pub fn with_runtime_dir(self, dir: impl Into<PathBuf>) -> Self {
        self.with_protected_dir(dir, RUNTIME_DIR_REASON)
    }

    /// Ergänzt ein `$XDG_CONFIG_HOME/humanitl`, das nicht unter `$HOME` liegt.
    #[must_use]
    pub fn with_config_dir(self, dir: impl Into<PathBuf>) -> Self {
        self.with_protected_dir(dir, CONFIG_DIR_REASON)
    }

    /// Ergänzt ein `$XDG_DATA_HOME/humanitl`, das nicht unter `$HOME` liegt.
    #[must_use]
    pub fn with_data_dir(self, dir: impl Into<PathBuf>) -> Self {
        self.with_protected_dir(dir, DATA_DIR_REASON)
    }

    /// Ergänzt einen ganzen Baum, der verboten ist, zum Beispiel ein
    /// `$XDG_CONFIG_HOME` oder `$XDG_DATA_HOME` außerhalb des
    /// Heimatverzeichnisses.
    ///
    /// `reason` steht im `why` des Befunds. Ein Pfad, der nicht absolut oder
    /// der `/` ist, taugt nicht als Basis und wird nicht aufgenommen.
    #[must_use]
    pub fn with_protected_dir(
        mut self,
        dir: impl Into<PathBuf>,
        reason: impl Into<String>,
    ) -> Self {
        let dir = dir.into();
        if is_meaningful_base(&dir) {
            let reason = reason.into();
            for spelling in spellings(&dir) {
                self.protected.push((spelling, reason.clone()));
            }
        }
        self
    }

    /// Prüft eine einzelne Host-Quelle aus dem Profil.
    ///
    /// `whence` ist der Schlüssel im Profil, zum Beispiel `mounts.extra_ro`; er
    /// steht im `why`, damit der Nutzer die Zeile findet.
    ///
    /// Ist die Quelle ein Verzeichnis, sucht ein begrenzter Suchlauf darunter
    /// nach einem Unix-Socket: Breitensuche bis [`SOCKET_WALK_MAX_DEPTH`], höchstens
    /// [`SOCKET_WALK_MAX_ENTRIES`] Einträge, Symlinks werden nicht verfolgt,
    /// unlesbare Verzeichnisse übersprungen. Ein Fund lehnt die Quelle ab; ein
    /// aufgebrauchtes Budget nicht.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `SANDBOX_006`, wenn der geschriebene oder der
    /// aufgelöste Pfad auf der Denylist steht, darüber liegt, ein Unix-Socket
    /// ist, einen enthält oder nicht absolut ist.
    pub fn check(&self, source: &Path, whence: &str) -> Result<(), Diagnostic> {
        self.check_scope(source, whence, Scope::Profile)
    }

    /// Prüft das Projektverzeichnis einer Sitzung.
    ///
    /// Anders als bei [`MountPolicy::check`] darf der Pfad unter dem
    /// Heimatverzeichnis liegen; das ist der Normalfall eines Projekts.
    /// Verboten bleibt alles andere: das Heimatverzeichnis selbst, seine
    /// Schlüsselverzeichnisse, die Denylist und jeder Pfad darüber. Für den
    /// Launcher (HUM-011), bevor `work_src` in den [`SessionContext`] kommt.
    ///
    /// Der Socket-Suchlauf aus [`MountPolicy::check`] läuft hier nicht: das
    /// Projekt gehört dem Nutzer, und ein liegen gebliebener Socket darin ist
    /// Sache des Isolation-Checks (HUM-041), nicht ein Grund, die Sitzung zu
    /// verweigern.
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `SANDBOX_006` und dem beanstandeten Pfad.
    pub fn check_work_dir(&self, source: &Path) -> Result<(), Diagnostic> {
        self.check_scope(source, "sandbox.work_dir", Scope::WorkDir)
    }

    fn check_scope(&self, source: &Path, whence: &str, scope: Scope) -> Result<(), Diagnostic> {
        if !source.is_absolute() {
            return Err(deny(
                whence,
                source,
                source,
                "a mount source must be an absolute path",
            ));
        }

        let (written, resolved) = resolve_candidates(source);
        for candidate in [&written, &resolved] {
            if let Some(reason) = self.reason_to_deny(candidate, scope) {
                return Err(deny(whence, source, candidate, &reason));
            }
        }
        if is_socket(&resolved) {
            return Err(deny(
                whence,
                source,
                &resolved,
                "a Unix socket on the host; the only socket in the sandbox is the proxy socket, and it comes from the session, not from the profile",
            ));
        }
        if scope == Scope::Profile
            && let Some(found) = find_socket_below(&resolved)
        {
            return Err(deny(
                whence,
                source,
                &resolved,
                &format!(
                    "contains the Unix socket {}; the only socket in the sandbox is the proxy socket bound by the launcher",
                    found.display()
                ),
            ));
        }
        Ok(())
    }

    fn reason_to_deny(&self, candidate: &Path, scope: Scope) -> Option<String> {
        for rule in FORBIDDEN_MOUNTS {
            let base = Path::new(rule.path);
            if candidate == base || (rule.whole_tree && candidate.starts_with(base)) {
                return Some(format!("{} is {}", rule.path, rule.reason));
            }
            if base.starts_with(candidate) {
                return Some(format!(
                    "{} contains {}, which is {}",
                    candidate.display(),
                    rule.path,
                    rule.reason
                ));
            }
        }

        for (dir, reason) in &self.protected {
            if candidate.starts_with(dir) {
                return Some(format!("{} is {reason}", dir.display()));
            }
            if dir.starts_with(candidate) {
                return Some(format!(
                    "{} contains {}, {reason}",
                    candidate.display(),
                    dir.display()
                ));
            }
        }

        for home in &self.homes {
            if candidate == home {
                return Some(format!(
                    "{} is the home directory itself; only the declared project directory is mounted into the sandbox",
                    home.display()
                ));
            }
            if home.starts_with(candidate) {
                return Some(format!(
                    "{} contains the home directory {}",
                    candidate.display(),
                    home.display()
                ));
            }
            for (relative, reason) in FORBIDDEN_IN_HOME {
                let base = home.join(relative);
                if candidate.starts_with(&base) {
                    return Some(format!("{} holds {reason}", base.display()));
                }
                if base.starts_with(candidate) {
                    return Some(format!(
                        "{} contains {}, which holds {reason}",
                        candidate.display(),
                        base.display()
                    ));
                }
            }
            if scope == Scope::Profile && candidate.starts_with(home) {
                return Some(format!(
                    "{} is inside the home directory; only the declared project directory is mounted into the sandbox",
                    home.display()
                ));
            }
        }
        None
    }
}

/// Ein Pfad taugt nur als Vergleichsbasis, wenn er absolut ist und nicht `/` ist.
fn is_meaningful_base(path: &Path) -> bool {
    path.is_absolute() && path.parent().is_some()
}

/// Ob der Pfad auf dem Host ein Unix-Socket ist. Ein Pfad, den es nicht gibt,
/// ist keiner.
fn is_socket(path: &Path) -> bool {
    std::fs::metadata(path).is_ok_and(|meta| meta.file_type().is_socket())
}

/// Der erste Unix-Socket unterhalb von `root`, wenn der begrenzte Suchlauf
/// einen findet.
///
/// Breitensuche, damit ein Socket nahe der Wurzel gefunden wird, bevor ein
/// großer Teilbaum das Budget aufbraucht: der übliche Fall ist ein
/// `dbus-…`-Socket direkt im eingehängten Verzeichnis, nicht einer tief unter
/// `/usr/lib`. Der Dateityp kommt aus dem Verzeichniseintrag selbst
/// (`DirEntry::file_type`), der Symlinks nicht auflöst; ein Verweis auf einen
/// Socket zählt deshalb nicht, ein Verweis auf ein Verzeichnis wird nicht
/// betreten. Was nicht lesbar ist, wird übersprungen. Ist `root` kein
/// Verzeichnis, gibt es nichts zu suchen.
fn find_socket_below(root: &Path) -> Option<PathBuf> {
    let mut queue: VecDeque<(PathBuf, usize)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));
    let mut seen = 0usize;
    while let Some((dir, depth)) = queue.pop_front() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            if seen >= SOCKET_WALK_MAX_ENTRIES {
                return None;
            }
            seen += 1;
            let Ok(entry) = entry else {
                continue;
            };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_socket() {
                return Some(entry.path());
            }
            if kind.is_dir() && depth + 1 < SOCKET_WALK_MAX_DEPTH {
                queue.push_back((entry.path(), depth + 1));
            }
        }
    }
    None
}

fn deny(whence: &str, source: &Path, candidate: &Path, reason: &str) -> Diagnostic {
    let resolved = if candidate == source {
        String::new()
    } else {
        format!(" (resolves to {})", candidate.display())
    };
    Diagnostic::builder(SANDBOX_006, Severity::Blocking)
        .why(format!(
            "{whence} names {}{resolved}: {reason}",
            source.display()
        ))
        .build()
}

/// Die Schreibweisen eines Pfades, die als Vergleichsbasis dienen: geschrieben
/// und aufgelöst, ohne Doppelung.
fn spellings(path: &Path) -> Vec<PathBuf> {
    let (written, resolved) = resolve_candidates(path);
    if written == resolved {
        vec![written]
    } else {
        vec![written, resolved]
    }
}

/// Der geschriebene und der aufgelöste Pfad, beide lexikalisch normalisiert.
///
/// Geprüft werden beide: der geschriebene Pfad fängt einen Mount, den ein
/// Symlink erst später gefährlich macht, der aufgelöste einen, der schon jetzt
/// woandershin zeigt.
fn resolve_candidates(source: &Path) -> (PathBuf, PathBuf) {
    let written = normalize(source);
    let resolved = normalize(&resolve_existing_prefix(source));
    (written, resolved)
}

/// Kanonisiert den längsten Teil des Pfades, den es schon gibt.
///
/// `std::fs::canonicalize` scheitert an einem Pfad, der noch nicht existiert.
/// Für die Prüfung genügt es, den vorhandenen Anfang aufzulösen und den Rest
/// wieder anzuhängen: ein Symlink kann nur im vorhandenen Teil liegen.
fn resolve_existing_prefix(source: &Path) -> PathBuf {
    let mut prefix = source.to_path_buf();
    let mut rest: Vec<OsString> = Vec::new();
    loop {
        if let Ok(real) = std::fs::canonicalize(&prefix) {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        let Some(name) = prefix.file_name().map(std::ffi::OsStr::to_os_string) else {
            return source.to_path_buf();
        };
        rest.push(name);
        if !prefix.pop() {
            return source.to_path_buf();
        }
    }
}

/// Entfernt `.` und rechnet `..` heraus, ohne das Dateisystem zu befragen.
pub(crate) fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() {
                    out.push("..");
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    if out.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        out
    }
}

impl SandboxProfile {
    /// Liest ein Profil aus einer Datei, ohne die Mount-Allowlist zu prüfen.
    ///
    /// Wer die Datei startet und nicht nur anzeigt, nimmt
    /// [`SandboxProfile::load_validated`].
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `CONFIG_001`, wenn die Datei fehlt, kein TOML ist oder
    /// einen unbekannten Schlüssel nennt, und mit `CONFIG_003`, `SANDBOX_007`,
    /// wenn ein Wert in sich unstimmig ist (siehe [`SandboxProfile::parse`]).
    pub fn load(path: &Path) -> Result<Self, Diagnostic> {
        let text = std::fs::read_to_string(path).map_err(|err| {
            Diagnostic::builder(CONFIG_001, Severity::Blocking)
                .why(format!("cannot read {}: {err}", path.display()))
                .build()
        })?;
        Self::parse(&text, path)
    }

    /// Liest ein Profil und prüft es vollständig, Mount-Allowlist eingeschlossen.
    ///
    /// Der Einstieg für den Launcher (HUM-011). Die Politik muss alle
    /// geschützten Verzeichnisse dieser Maschine kennen, also aus
    /// `humanitl_config::Paths` gebaut sein ([`MountPolicy::from_paths`]), nicht
    /// aus dem Heimatverzeichnis allein: `$XDG_RUNTIME_DIR`,
    /// `$XDG_CONFIG_HOME/humanitl` und `$XDG_DATA_HOME/humanitl` können
    /// außerhalb von `/run` und `$HOME` liegen.
    ///
    /// # Errors
    ///
    /// Wie [`SandboxProfile::load`], zusätzlich `SANDBOX_006`.
    pub fn load_validated(path: &Path, policy: &MountPolicy) -> Result<Self, Diagnostic> {
        let profile = Self::load(path)?;
        profile.validate_with(policy)?;
        Ok(profile)
    }

    /// Liest ein Profil aus dem Text einer TOML-Datei.
    ///
    /// `source` benennt nur die Herkunft für die Fehlermeldung.
    ///
    /// # Errors
    ///
    /// - `CONFIG_001`, wenn der Text kein TOML ist oder einen unbekannten Schlüssel nennt,
    /// - `CONFIG_003`, wenn ein Wert unzulässig ist oder einem anderen widerspricht,
    /// - `SANDBOX_007`, wenn eine Bridge nach außen zeigt.
    pub fn parse(text: &str, source: &Path) -> Result<Self, Diagnostic> {
        Self::parse_with_floor(text, source, SocketFloor::Strict)
    }

    /// Wie [`SandboxProfile::parse`], mit einem anderen Socket-Boden.
    ///
    /// Die einzige Stelle, an der der Filter des Shims weiter wird als
    /// [`SocketFloor::Strict`], und sie liegt im Code, nicht in einer Datei
    /// (siehe [`SocketFloor`]). Aufrufer außerhalb des Profils `browser`
    /// nehmen [`SandboxProfile::parse`].
    ///
    /// # Errors
    ///
    /// Wie [`SandboxProfile::parse`].
    pub fn parse_with_floor(
        text: &str,
        source: &Path,
        floor: SocketFloor,
    ) -> Result<Self, Diagnostic> {
        let mut profile: Self = toml::from_str(text).map_err(|err: toml::de::Error| {
            // serde meldet `deny_unknown_fields` als "unknown field `x`, …";
            // das ist der Fall des Registers für `CONFIG_002`. Ändert sich der
            // Wortlaut, bleibt es ein `CONFIG_001`, nicht ein stiller Erfolg.
            let code = if err.message().starts_with("unknown field") {
                CONFIG_002
            } else {
                CONFIG_001
            };
            Diagnostic::builder(code, Severity::Blocking)
                .why(format!(
                    "{} is not a valid profile: {err}",
                    source.display()
                ))
                .build()
        })?;
        profile.seccomp.deny_syscalls = union_deny_syscalls(&profile.seccomp.deny_syscalls);
        profile.check_consistency(source, floor)?;
        // Nach der Prüfung sind die beiden Listen mengengleich mit dem Boden;
        // hier bekommen sie zusätzlich dessen Reihenfolge, ohne Doppelungen,
        // damit `HUMANITL_SECCOMP_FAMILIES` für dasselbe Profil immer gleich
        // aussieht ([`crate::bridge_env::shim_env`]).
        profile.seccomp.allow_families = floor.families();
        profile.seccomp.allow_types = floor.types();
        Ok(profile)
    }

    /// Prüft jede Host-Quelle des Profils gegen die Politik, und danach, dass
    /// die drei Dateien der Sitzung einen Mountpoint bekommen können.
    ///
    /// Das Projektverzeichnis der Sitzung ist nicht dabei; der Launcher prüft es
    /// mit [`MountPolicy::check_work_dir`].
    ///
    /// # Errors
    ///
    /// [`Diagnostic`] mit `SANDBOX_006` und dem beanstandeten Pfad, oder mit
    /// `CONFIG_003`, wenn Proxy-Socket, CA oder Shim unter einem Host-Bind
    /// liegen, in dem `bwrap` den Mountpoint nicht anlegen kann.
    pub fn validate_with(&self, policy: &MountPolicy) -> Result<(), Diagnostic> {
        for (whence, source) in self.mount_sources() {
            policy.check(source, whence)?;
        }
        self.check_session_mountpoints()
    }

    /// Jede Host-Quelle des Profils, mit dem Schlüssel, unter dem sie steht.
    ///
    /// Das Projektverzeichnis steht nicht dabei: es kommt aus dem
    /// [`SessionContext`], nicht aus dem Profil. Wer es prüfen will, ruft
    /// [`MountPolicy::check`] selbst auf.
    pub fn mount_sources(&self) -> impl Iterator<Item = (&'static str, &Path)> {
        let ro = self.mounts.ro.iter().map(|p| ("mounts.ro", p.as_path()));
        let extra_read_only = self
            .mounts
            .extra_ro
            .iter()
            .map(|p| ("mounts.extra_ro", p.as_path()));
        let extra_writable = self
            .mounts
            .extra_rw
            .iter()
            .map(|p| ("mounts.extra_rw", p.as_path()));
        ro.chain(extra_read_only).chain(extra_writable)
    }

    /// Der Modus, mit dem `/work` eingehängt wird: der strengere der beiden.
    ///
    /// Ein Profil, das `ro` sagt, lässt sich von der Sitzung nicht aufweichen.
    /// Andersherum schon: `--work-mode ro` auf der Kommandozeile gilt auch für
    /// ein Profil, das `rw` sagt.
    #[must_use]
    pub fn effective_work_mode(&self, session: WorkMode) -> WorkMode {
        match (self.mounts.work.mode, session) {
            (Some(WorkMode::Ro), _) | (_, WorkMode::Ro) => WorkMode::Ro,
            _ => WorkMode::Rw,
        }
    }

    /// Die Bridge, die den Proxy erreichbar macht.
    #[must_use]
    pub fn proxy_bridge(&self) -> Option<&Bridge> {
        self.network
            .bridges
            .iter()
            .find(|bridge| bridge.name == PROXY_BRIDGE)
    }

    /// Die Dateien, die die Kommandozeile überdeckt: [`MANDATORY_MASKED_FILES`]
    /// zuerst, dann die Ergänzungen aus `mounts.masked_files`, ohne Doppelungen.
    ///
    /// Ein Profil kann Dateien hinzufügen, die beiden Pflichteinträge aber
    /// nicht streichen; `masked_files = []` ergibt genau die Pflichteinträge.
    #[must_use]
    pub fn effective_masked_files(&self) -> Vec<PathBuf> {
        let mut out: Vec<PathBuf> = MANDATORY_MASKED_FILES.iter().map(PathBuf::from).collect();
        for path in &self.mounts.masked_files {
            if !out.contains(path) {
                out.push(path.clone());
            }
        }
        out
    }

    fn check_consistency(&self, source: &Path, floor: SocketFloor) -> Result<(), Diagnostic> {
        self.check_header(source)?;
        self.check_floors(source, floor)?;

        let where_ = source.display();
        for (key, path) in self.sandbox_paths() {
            if !path.is_absolute() {
                return Err(range(format!(
                    "{key} = {} is not an absolute path in the sandbox",
                    path.display()
                )));
            }
        }

        for bridge in &self.network.bridges {
            if bridge.dir == BridgeDirection::Out {
                return Err(Diagnostic::builder(SANDBOX_007, Severity::Blocking)
                    .why(format!(
                        "{where_}: bridge {:?}: direction out not supported yet",
                        bridge.name
                    ))
                    .build());
            }
        }

        self.check_bridges(source)?;
        Ok(())
    }

    /// Genau eine Bridge, und genau die Proxy-Bridge.
    ///
    /// Die zweite Garantie ist „genau eine Tür". Der Shim öffnet jede Bridge,
    /// die in `HUMANITL_BRIDGES` steht ([`crate::bridge_env::shim_env`]), also
    /// entscheidet diese Prüfung, wie viele Türen es gibt. Eine zweite Bridge
    /// mit `dir = "in"` wäre ein zweiter Listener auf einen zweiten
    /// Unix-Socket und damit ein Weg an der Aufzeichnung vorbei; `dir = "out"`
    /// hat die Richtungsprüfung vorher schon abgelehnt (`SANDBOX_007`).
    ///
    /// Verglichen wird mit [`Bridge::proxy_on`]: Name, Richtung,
    /// Loopback-Adresse mit `network.proxy_port` und
    /// [`PROXY_SOCKET_DST`] als Ziel. Jede Abweichung ist `CONFIG_003` und
    /// nennt Schlüssel und beanstandeten Wert.
    fn check_bridges(&self, source: &Path) -> Result<(), Diagnostic> {
        let where_ = source.display();
        let expected = Bridge::proxy_on(self.network.proxy_port);

        let bridge = match self.network.bridges.as_slice() {
            [only] => only,
            [] => {
                return Err(range(format!(
                    "{where_}: network.bridges is empty; it must hold exactly the bridge {PROXY_BRIDGE:?} on {}, nothing would reach the proxy",
                    expected.listen
                )));
            }
            many => {
                let names: Vec<&str> = many.iter().map(|b| b.name.as_str()).collect();
                return Err(range(format!(
                    "{where_}: network.bridges has {} entries ({}); exactly one is allowed, the bridge {PROXY_BRIDGE:?}: every further bridge is a second door out of the sandbox",
                    many.len(),
                    names.join(", ")
                )));
            }
        };

        if bridge.name != expected.name {
            return Err(range(format!(
                "{where_}: network.bridges names the bridge {:?}; the only bridge this build serves is {PROXY_BRIDGE:?}",
                bridge.name
            )));
        }
        if bridge.dir != expected.dir {
            return Err(range(format!(
                "{where_}: bridge {PROXY_BRIDGE:?} has dir = {:?}; it must be {:?}",
                bridge.dir.as_str(),
                expected.dir.as_str()
            )));
        }
        if bridge.listen != expected.listen {
            return Err(range(format!(
                "{where_}: bridge {PROXY_BRIDGE:?} listens on {}; it must listen on {} (network.proxy_port = {})",
                bridge.listen, expected.listen, self.network.proxy_port
            )));
        }
        if bridge.socket != expected.socket {
            return Err(range(format!(
                "{where_}: bridge {PROXY_BRIDGE:?} serves {}; the one door of the sandbox is {}",
                bridge.socket.display(),
                expected.socket.display()
            )));
        }
        if self.network.proxy_socket_dst != expected.socket {
            return Err(range(format!(
                "{where_}: network.proxy_socket_dst = {}; it must be {}, the path the bridge serves",
                self.network.proxy_socket_dst.display(),
                expected.socket.display()
            )));
        }
        Ok(())
    }

    /// Die festen Angaben: Version, Name und die Schalter der Sandbox, die
    /// kein Profil umlegen darf.
    fn check_header(&self, source: &Path) -> Result<(), Diagnostic> {
        let where_ = source.display();

        if self.version != PROFILE_VERSION {
            return Err(range(format!(
                "{where_}: version = {}, this build reads version {PROFILE_VERSION}",
                self.version
            )));
        }
        if self.name.trim().is_empty() {
            return Err(range(format!("{where_}: name is empty")));
        }
        if self.sandbox.backend != "bwrap" {
            return Err(range(format!(
                "{where_}: sandbox.backend = {:?}, the only backend of this build is \"bwrap\"",
                self.sandbox.backend
            )));
        }
        if self.sandbox.hostname != HOSTNAME {
            return Err(range(format!(
                "{where_}: sandbox.hostname = {:?}, it must be {HOSTNAME:?}",
                self.sandbox.hostname
            )));
        }
        if !self.sandbox.die_with_parent {
            return Err(range(format!(
                "{where_}: sandbox.die_with_parent must not be false; a sandbox that outlives the daemon is unsupervised"
            )));
        }
        if !self.sandbox.new_session {
            return Err(range(format!(
                "{where_}: sandbox.new_session must not be false; without it TIOCSTI writes into the user's terminal"
            )));
        }
        for required in Namespace::ALL {
            if !self.sandbox.unshare.contains(&required) {
                return Err(range(format!(
                    "{where_}: sandbox.unshare misses {:?}; the list spells out --unshare-all and cannot shrink: {}",
                    required.as_str(),
                    required.why_required()
                )));
            }
        }
        Ok(())
    }

    /// Die Böden: die Syscalls und tmpfs-Pfade, unter die kein Profil fallen
    /// darf, und die Socket-Politik, die es weder unter- noch überschreiten
    /// darf.
    fn check_floors(&self, source: &Path, floor: SocketFloor) -> Result<(), Diagnostic> {
        let where_ = source.display();

        // Die Socket-Politik ist der einzige Boden, der in beide Richtungen
        // wirkt. `deny_syscalls` kann wachsen, ohne dass eine Garantie leidet;
        // eine Familie oder ein Typ mehr ist dagegen genau die Aufweichung,
        // die die dritte Garantie behauptet zu verhindern (`AF_UNIX` neben dem
        // Proxy-Socket, `SOCK_DGRAM` für DNS und QUIC).
        let families = floor.families();
        for family in &self.seccomp.allow_families {
            if !families.contains(family) {
                return Err(range(format!(
                    "{where_}: seccomp.allow_families names {:?}; the filter allows exactly {}, and no profile can widen that",
                    family.as_str(),
                    names(&families, SocketFamily::as_str)
                )));
            }
        }
        for required in &families {
            if !self.seccomp.allow_families.contains(required) {
                return Err(range(format!(
                    "{where_}: seccomp.allow_families misses {:?}; the filter allows exactly {}, and no profile can narrow that either: without it the agent cannot reach the proxy",
                    required.as_str(),
                    names(&families, SocketFamily::as_str)
                )));
            }
        }
        let types = floor.types();
        for kind in &self.seccomp.allow_types {
            if !types.contains(kind) {
                return Err(range(format!(
                    "{where_}: seccomp.allow_types names {:?}; the filter allows exactly {}, and no profile can widen that",
                    kind.as_str(),
                    names(&types, SocketType::as_str)
                )));
            }
        }
        for required in &types {
            if !self.seccomp.allow_types.contains(required) {
                return Err(range(format!(
                    "{where_}: seccomp.allow_types misses {:?}; the filter allows exactly {}",
                    required.as_str(),
                    names(&types, SocketType::as_str)
                )));
            }
        }

        // Sicherheitsnetz: nach `union_deny_syscalls` kann hier nichts fehlen,
        // aber ein Profil, das ohne `parse` gebaut wurde, soll nicht durchrutschen.
        for required in DEFAULT_DENY_SYSCALLS {
            if !self
                .seccomp
                .deny_syscalls
                .iter()
                .any(|name| name == required)
            {
                return Err(range(format!(
                    "{where_}: seccomp.deny_syscalls misses {required:?}; the list can grow but not fall below the built-in floor"
                )));
            }
        }
        for required in REQUIRED_TMPFS {
            if !self
                .mounts
                .tmpfs
                .iter()
                .any(|path| path == Path::new(required))
            {
                return Err(range(format!(
                    "{where_}: mounts.tmpfs misses {required:?}; without it the sandbox sees the host's {required}"
                )));
            }
        }
        Ok(())
    }

    /// Die drei Dateien, die die Sitzung einhängt (Proxy-Socket, CA, Shim),
    /// brauchen einen Mountpoint, den `bwrap` anlegen kann.
    ///
    /// Unter einem nur lesbaren Bind aus `mounts.ro` oder `mounts.extra_ro`
    /// scheitert das mit `EROFS` (`/usr/local/bin/humanitl-shim` unter dem
    /// `--ro-bind /usr`, gemessen mit bubblewrap 0.11); unter einem
    /// beschreibbaren Bind aus `mounts.extra_rw` entstünde die leere Datei auf
    /// dem Host. Beides ist `CONFIG_003`, es sei denn, ein `mounts.tmpfs`
    /// unterhalb des Binds deckt den Ort ab; das tmpfs kommt in der Zeile nach
    /// den Binds und ist beschreibbar. Deshalb liegen Shim und Socket unter
    /// `/run/humanitl`, das nie vom Host kommt ([`SHIM_DST`]).
    ///
    /// Läuft in [`SandboxProfile::validate_with`], nach der Politik: ein
    /// `extra_ro = ["/run"]` ist zuerst ein verbotener Mount (`SANDBOX_006`)
    /// und erst dann ein unbrauchbarer Mountpoint.
    fn check_session_mountpoints(&self) -> Result<(), Diagnostic> {
        let targets = [
            (
                "network.proxy_socket_dst",
                self.network.proxy_socket_dst.as_path(),
            ),
            ("network.ca_cert_dst", self.network.ca_cert_dst.as_path()),
            ("network.shim_dst", self.network.shim_dst.as_path()),
        ];
        let binds = self
            .mounts
            .ro
            .iter()
            .map(|p| ("mounts.ro", "read-only: EROFS", p.as_path()))
            .chain(
                self.mounts
                    .extra_ro
                    .iter()
                    .map(|p| ("mounts.extra_ro", "read-only: EROFS", p.as_path())),
            )
            .chain(self.mounts.extra_rw.iter().map(|p| {
                (
                    "mounts.extra_rw",
                    "writable: an empty file would appear on the host",
                    p.as_path(),
                )
            }));
        for (whence, consequence, bind) in binds {
            for (key, dst) in targets {
                if !dst.starts_with(bind) {
                    continue;
                }
                let covered = self
                    .mounts
                    .tmpfs
                    .iter()
                    .any(|tmpfs| tmpfs.starts_with(bind) && dst.starts_with(tmpfs));
                if covered {
                    continue;
                }
                return Err(range(format!(
                    "profile {:?}: {key} = {} lies under the host bind {} from {whence}; bwrap cannot create the mount point there ({consequence}), and no mounts.tmpfs below {} covers it",
                    self.name,
                    dst.display(),
                    bind.display(),
                    bind.display()
                )));
            }
        }
        Ok(())
    }

    /// Alle Ziele in der Sandbox, mit ihrem Schlüssel. Müssen absolut sein.
    fn sandbox_paths(&self) -> Vec<(String, &Path)> {
        let mut out: Vec<(String, &Path)> = vec![
            ("mounts.work.dst".to_owned(), self.mounts.work.dst.as_path()),
            (
                "network.proxy_socket_dst".to_owned(),
                self.network.proxy_socket_dst.as_path(),
            ),
            (
                "network.ca_cert_dst".to_owned(),
                self.network.ca_cert_dst.as_path(),
            ),
            (
                "network.shim_dst".to_owned(),
                self.network.shim_dst.as_path(),
            ),
        ];
        out.extend(
            self.mounts
                .tmpfs
                .iter()
                .map(|p| ("mounts.tmpfs".to_owned(), p.as_path())),
        );
        out.extend(
            self.mounts
                .masked_files
                .iter()
                .map(|p| ("mounts.masked_files".to_owned(), p.as_path())),
        );
        out.extend(
            self.mounts
                .symlinks
                .iter()
                .map(|s| ("mounts.symlinks".to_owned(), s.link.as_path())),
        );
        if let Some(proc) = self.mounts.proc.as_deref() {
            out.push(("mounts.proc".to_owned(), proc));
        }
        if let Some(dev) = self.mounts.dev.as_deref() {
            out.push(("mounts.dev".to_owned(), dev));
        }
        out
    }
}

/// Eine Aufzählung für den Befund: `"AF_INET", "AF_INET6"`.
fn names<T: Copy>(values: &[T], name: fn(T) -> &'static str) -> String {
    values
        .iter()
        .map(|value| format!("{:?}", name(*value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn range(why: String) -> Diagnostic {
    Diagnostic::builder(CONFIG_003, Severity::Blocking)
        .why(why)
        .build()
}

/// [`DEFAULT_DENY_SYSCALLS`] zuerst, dann die Ergänzungen des Profils in ihrer
/// Reihenfolge, jeder Name einmal.
fn union_deny_syscalls(from_profile: &[String]) -> Vec<String> {
    let mut out: Vec<String> = DEFAULT_DENY_SYSCALLS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    for name in from_profile {
        if !out.contains(name) {
            out.push(name.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::{Path, PathBuf};

    use humanitl_config::Env;

    use super::{
        BridgeDirection, DEFAULT_DENY_SYSCALLS, MANDATORY_MASKED_FILES, MountPolicy, Namespace,
        SandboxProfile, SocketFamily, SocketFloor, SocketType, WorkMode, normalize,
    };

    const MINIMAL: &str = r#"
version = 1
name = "minimal"
"#;

    fn parse(text: &str) -> Result<SandboxProfile, humanitl_core::Diagnostic> {
        SandboxProfile::parse(text, Path::new("<test>"))
    }

    #[test]
    fn defaults_fill_a_minimal_profile() {
        let profile = parse(MINIMAL).expect("minimal profile parses");
        assert_eq!(profile.sandbox.backend, "bwrap");
        assert!(profile.sandbox.unshare.contains(&Namespace::Net));
        assert_eq!(profile.network.proxy_port, 3128);
        assert_eq!(profile.network.bridges.len(), 1);
        assert_eq!(profile.network.bridges[0].dir, BridgeDirection::In);
        assert_eq!(
            profile.seccomp.allow_families,
            vec![SocketFamily::AfInet, SocketFamily::AfInet6]
        );
        assert_eq!(profile.seccomp.allow_types, vec![SocketType::SockStream]);
        assert_eq!(
            profile.seccomp.deny_syscalls.len(),
            DEFAULT_DENY_SYSCALLS.len()
        );
    }

    #[test]
    fn version_two_is_out_of_range() {
        let err = parse("version = 2\nname = \"x\"\n").expect_err("version 2 is unknown");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("version = 2"), "{}", err.why);
    }

    #[test]
    fn a_profile_without_net_namespace_is_refused() {
        let text = "version = 1\nname = \"x\"\n[sandbox]\nunshare = [\"user\", \"pid\"]\n";
        let err = parse(text).expect_err("net is not optional");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("net"), "{}", err.why);
    }

    #[test]
    fn a_foreign_backend_is_refused() {
        let text = "version = 1\nname = \"x\"\n[sandbox]\nbackend = \"docker\"\n";
        let err = parse(text).expect_err("docker is not a backend of this build");
        assert_eq!(err.code.as_str(), "CONFIG_003");
    }

    #[test]
    fn a_profile_that_keeps_the_pid_namespace_is_refused() {
        let text = concat!(
            "version = 1\nname = \"x\"\n[sandbox]\n",
            "unshare = [\"user\", \"net\", \"ipc\", \"uts\", \"cgroup\"]\n"
        );
        let err = parse(text).expect_err("the list spells out --unshare-all");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("pid"), "{}", err.why);
        assert!(err.why.contains("/proc"), "{}", err.why);
    }

    #[test]
    fn deny_syscalls_cannot_fall_below_the_floor() {
        // Ein Profil, das nur eine Ergänzung nennt, erweitert die Liste, es
        // ersetzt sie nicht: erst der Boden, dann die Ergänzung.
        let text = "version = 1\nname = \"x\"\n[seccomp]\ndeny_syscalls = [\"mount\"]\n";
        let profile = parse(text).expect("an addition extends the floor");
        assert_eq!(
            profile.seccomp.deny_syscalls.len(),
            DEFAULT_DENY_SYSCALLS.len() + 1
        );
        let (floor, added) = profile
            .seccomp
            .deny_syscalls
            .split_at(DEFAULT_DENY_SYSCALLS.len());
        assert_eq!(floor, DEFAULT_DENY_SYSCALLS);
        assert_eq!(added, ["mount"]);

        // Ein Teil des Bodens, wiederholt und umgeordnet, ändert nichts.
        let text = "version = 1\nname = \"x\"\n[seccomp]\ndeny_syscalls = [\"keyctl\", \"ptrace\", \"keyctl\"]\n";
        let profile = parse(text).expect("names from the floor are deduplicated");
        assert_eq!(profile.seccomp.deny_syscalls, DEFAULT_DENY_SYSCALLS);

        // Eine leere Liste ist der Boden.
        let text = "version = 1\nname = \"x\"\n[seccomp]\ndeny_syscalls = []\n";
        let profile = parse(text).expect("an empty list means the floor");
        assert_eq!(profile.seccomp.deny_syscalls, DEFAULT_DENY_SYSCALLS);

        // Das Sicherheitsnetz in `check_consistency` bleibt: ein Profil, das an
        // `parse` vorbei gebaut wurde, fällt dort auf.
        let mut profile = parse(MINIMAL).expect("minimal profile parses");
        profile.seccomp.deny_syscalls = vec!["ptrace".to_owned()];
        let err = profile
            .check_consistency(Path::new("<test>"), SocketFloor::Strict)
            .expect_err("io_uring is not optional");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("io_uring_setup"), "{}", err.why);
    }

    /// Ein Mountpoint, den `bwrap` nicht anlegen kann, fällt beim Laden auf,
    /// nicht erst beim Start mit `EROFS`.
    #[test]
    fn a_session_file_under_a_host_bind_needs_a_tmpfs() {
        let text = concat!(
            "version = 1\nname = \"x\"\n[mounts]\nro = [\"/usr\"]\n",
            "[network]\nshim_dst = \"/usr/local/bin/humanitl-shim\"\n"
        );
        let policy = MountPolicy::new("/home/u");
        let validate = |text: &str| parse(text).expect("parses").validate_with(&policy);
        let err = validate(text).expect_err("bwrap cannot create a file under a read-only bind");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("network.shim_dst"), "{}", err.why);
        assert!(err.why.contains("mounts.ro"), "{}", err.why);
        assert!(err.why.contains("EROFS"), "{}", err.why);

        let text = concat!(
            "version = 1\nname = \"x\"\n[mounts]\nro = [\"/usr\"]\n",
            "tmpfs = [\"/tmp\", \"/dev/shm\", \"/usr/local/bin\"]\n",
            "[network]\nshim_dst = \"/usr/local/bin/humanitl-shim\"\n"
        );
        validate(text).expect("a tmpfs below the bind is a place bwrap can create the mount point");

        let text = concat!(
            "version = 1\nname = \"x\"\n[mounts]\nextra_rw = [\"/srv/data\"]\n",
            "[network]\nca_cert_dst = \"/srv/data/ca.crt\"\n"
        );
        let err = validate(text).expect_err("a writable bind would get an empty file on the host");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("network.ca_cert_dst"), "{}", err.why);
        assert!(err.why.contains("on the host"), "{}", err.why);

        // Die ausgelieferten Ziele liegen unter /run und /etc/humanitl, die
        // nie vom Host kommen.
        validate(MINIMAL).expect("the defaults need no tmpfs");
    }

    #[test]
    fn a_foreign_hostname_is_refused() {
        let text = "version = 1\nname = \"x\"\n[sandbox]\nhostname = \"laptop\"\n";
        let err = parse(text).expect_err("the hostname is not a choice");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("sandbox.hostname"), "{}", err.why);
        assert!(err.why.contains("laptop"), "{}", err.why);
    }

    #[test]
    fn die_with_parent_and_new_session_cannot_be_false() {
        let text = "version = 1\nname = \"x\"\n[sandbox]\ndie_with_parent = false\n";
        let err = parse(text).expect_err("a sandbox must die with the daemon");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(
            err.why
                .contains("sandbox.die_with_parent must not be false"),
            "{}",
            err.why
        );

        let text = "version = 1\nname = \"x\"\n[sandbox]\nnew_session = false\n";
        let err = parse(text).expect_err("a sandbox must not share the user's session");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(
            err.why.contains("sandbox.new_session must not be false"),
            "{}",
            err.why
        );
    }

    #[test]
    fn tmpfs_must_cover_tmp_and_dev_shm() {
        let text = "version = 1\nname = \"x\"\n[mounts]\ntmpfs = [\"/tmp\"]\n";
        let err = parse(text).expect_err("/dev/shm is not optional");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("mounts.tmpfs"), "{}", err.why);
        assert!(err.why.contains("/dev/shm"), "{}", err.why);

        let text = "version = 1\nname = \"x\"\n[mounts]\ntmpfs = [\"/dev/shm\"]\n";
        let err = parse(text).expect_err("/tmp is not optional");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("/tmp"), "{}", err.why);

        let text =
            "version = 1\nname = \"x\"\n[mounts]\ntmpfs = [\"/dev/shm\", \"/tmp\", \"/var/tmp\"]\n";
        parse(text).expect("both present, in any order, with company");
    }

    #[test]
    fn masked_files_always_include_the_mandatory_ones() {
        let text = "version = 1\nname = \"x\"\n[mounts]\nmasked_files = []\n";
        let profile = parse(text).expect("an empty list parses");
        assert_eq!(
            profile.effective_masked_files(),
            MANDATORY_MASKED_FILES
                .iter()
                .map(PathBuf::from)
                .collect::<Vec<_>>()
        );

        let text = "version = 1\nname = \"x\"\n[mounts]\nmasked_files = [\"/work/.npmrc\", \"/work/.envrc\"]\n";
        let profile = parse(text).expect("additions parse");
        assert_eq!(
            profile.effective_masked_files(),
            ["/work/.envrc", "/work/.git/config", "/work/.npmrc"].map(PathBuf::from)
        );
    }

    #[test]
    fn from_dirs_protects_every_directory_with_its_parents() {
        let policy = MountPolicy::from_dirs(
            "/home/u",
            "/var/lib/rt-u",
            "/mnt/cfg/humanitl",
            "/mnt/data/humanitl",
        );
        for (path, expect) in [
            ("/var/lib/rt-u/bus", "runtime directory"),
            ("/var/lib", "runtime directory"),
            ("/mnt/cfg/humanitl/rules.yaml", "configuration directory"),
            ("/mnt/cfg", "configuration directory"),
            ("/mnt/data/humanitl/ca/ca.key", "data directory"),
            // `/mnt` contains both; whichever is named first is a correct answer.
            ("/mnt", "Humanitl's own"),
            ("/home/u/.ssh", "private keys"),
        ] {
            let Err(err) = policy.check(Path::new(path), "mounts.extra_ro") else {
                panic!("{path} must be forbidden");
            };
            assert_eq!(err.code.as_str(), "SANDBOX_006", "{path}");
            assert!(err.why.contains(expect), "{path}: {}", err.why);
        }
        policy
            .check(Path::new("/mnt/cfg/other"), "mounts.extra_ro")
            .expect("a sibling of a protected directory stays allowed");
        policy
            .check(Path::new("/mnt/data/other"), "mounts.extra_ro")
            .expect("a sibling of a protected directory stays allowed");
    }

    #[test]
    fn from_env_reads_the_xdg_variables_from_the_given_map_only() {
        let env = Env::from_pairs([
            ("HOME", "/home/u"),
            ("XDG_RUNTIME_DIR", "/var/lib/rt-u"),
            ("XDG_CONFIG_HOME", "/mnt/cfg"),
            ("XDG_DATA_HOME", "/mnt/data"),
        ])
        .with_uid(1000);
        let policy = MountPolicy::from_env(&env);
        assert_eq!(
            policy,
            MountPolicy::from_dirs(
                "/home/u",
                "/var/lib/rt-u/humanitl",
                "/mnt/cfg/humanitl",
                "/mnt/data/humanitl",
            )
            .with_runtime_dir("/var/lib/rt-u")
        );
        for path in [
            "/var/lib/rt-u/wayland-0",
            "/mnt/cfg/humanitl",
            "/mnt/data/humanitl/blobs",
        ] {
            let err = policy
                .check(Path::new(path), "mounts.extra_rw")
                .expect_err("the XDG directories are protected wherever they are");
            assert_eq!(err.code.as_str(), "SANDBOX_006", "{path}");
        }

        // Ohne XDG-Variablen liegen alle drei unter $HOME oder /run und sind
        // dort schon verboten; der Ersatz unter /tmp wird trotzdem geschützt.
        let env = Env::from_pairs([("HOME", "/home/u")]).with_uid(1000);
        let policy = MountPolicy::from_env(&env);
        policy
            .check(Path::new("/opt/toolchain"), "mounts.extra_ro")
            .expect("nothing else is touched");
        let err = policy
            .check(Path::new("/home/u/.config/humanitl"), "mounts.extra_ro")
            .expect_err("the config dir under home is forbidden either way");
        assert_eq!(err.code.as_str(), "SANDBOX_006");
    }

    #[test]
    fn the_work_dir_may_live_under_home_but_not_be_it() {
        let policy = MountPolicy::new("/home/u");
        policy
            .check_work_dir(Path::new("/home/u/projects/app"))
            .expect("a project below home is the normal case");
        policy
            .check_work_dir(Path::new("/tmp/escape/work"))
            .expect("a project in a temporary directory is fine too");
        for bad in [
            "/home/u",
            "/home",
            "/",
            "/home/u/.ssh",
            "/home/u/.config",
            "/home/u/.local/share/humanitl/blobs",
            "/run/user/1000",
            "/var",
        ] {
            let Err(err) = policy.check_work_dir(Path::new(bad)) else {
                panic!("{bad} must not be a project directory");
            };
            assert_eq!(err.code.as_str(), "SANDBOX_006", "{bad}");
        }
    }

    #[test]
    fn a_bridge_port_that_contradicts_proxy_port_is_refused() {
        let text = concat!(
            "version = 1\nname = \"x\"\n[network]\nproxy_port = 3128\n",
            "bridges = [{ name = \"proxy\", dir = \"in\", listen = \"127.0.0.1:9999\", ",
            "socket = \"/run/humanitl/proxy.sock\" }]\n"
        );
        let err = parse(text).expect_err("the ports must agree");
        assert_eq!(err.code.as_str(), "CONFIG_003");
        assert!(err.why.contains("9999"), "{}", err.why);
    }

    #[test]
    fn a_profile_without_proxy_bridge_is_refused() {
        let text = "version = 1\nname = \"x\"\n[network]\nbridges = []\n";
        let err = parse(text).expect_err("without a bridge nothing reaches the proxy");
        assert_eq!(err.code.as_str(), "CONFIG_003");
    }

    /// Ein Profil, dessen `work`-Mount den angegebenen Modus trägt.
    fn with_work_mode(mode: &str) -> SandboxProfile {
        let text = format!(
            "version = 1\nname = \"x\"\n[mounts]\nwork = {{ dst = \"/work\", mode = \"{mode}\" }}\n"
        );
        parse(&text).expect("the probe profile parses")
    }

    #[test]
    fn work_mode_takes_the_stricter_of_profile_and_session() {
        let rw = with_work_mode("rw");
        assert_eq!(rw.effective_work_mode(WorkMode::Rw), WorkMode::Rw);
        assert_eq!(rw.effective_work_mode(WorkMode::Ro), WorkMode::Ro);

        let ro = with_work_mode("ro");
        assert_eq!(ro.effective_work_mode(WorkMode::Rw), WorkMode::Ro);
        assert_eq!(ro.effective_work_mode(WorkMode::Ro), WorkMode::Ro);
    }

    #[test]
    fn normalize_resolves_dot_and_dotdot() {
        assert_eq!(
            normalize(Path::new("/home/u/../u/./.ssh")),
            PathBuf::from("/home/u/.ssh")
        );
        assert_eq!(normalize(Path::new("/a/b/../..")), PathBuf::from("/"));
    }

    #[test]
    fn a_relative_source_is_refused() {
        let policy = MountPolicy::new("/home/u");
        let err = policy
            .check(Path::new("etc/ssl"), "mounts.ro")
            .expect_err("relative sources are ambiguous");
        assert_eq!(err.code.as_str(), "SANDBOX_006");
    }

    #[test]
    fn a_custom_runtime_dir_is_forbidden() {
        let policy = MountPolicy::new("/home/u").with_runtime_dir("/var/lib/run-u");
        let err = policy
            .check(Path::new("/var/lib/run-u/bus"), "mounts.extra_ro")
            .expect_err("the runtime directory holds the session bus");
        assert_eq!(err.code.as_str(), "SANDBOX_006");
        assert!(err.why.contains("/var/lib/run-u"), "{}", err.why);
    }

    #[test]
    fn a_root_home_does_not_forbid_everything() {
        let policy = MountPolicy::new("/");
        policy
            .check(Path::new("/usr"), "mounts.ro")
            .expect("/usr stays allowed even with a nonsensical home");
    }
}
