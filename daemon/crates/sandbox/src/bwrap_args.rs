//! Vom Profil zur Kommandozeile.
//!
//! [`SandboxProfile::to_bwrap_args`] erzeugt aus Profil, [`SessionContext`] und
//! [`LaunchInputs`] genau die Argumente, mit denen `bwrap` startet — nichts
//! wird beim Start noch dazugetan. Deshalb ist die Zeile, die die Oberfläche
//! unter „Sandbox" zeigt (`humanitl sandbox argv`), die Wahrheit und nicht eine
//! Beschreibung davon.
//!
//! Die Reihenfolge steht fest (HUM-010, erweitert in HUM-011) und wird von
//! `tests/snapshots/default.argv.txt` festgehalten:
//!
//! 1. `--unshare-*` in der festen Reihenfolge von [`Namespace::ALL`], nicht
//!    der des Profils, damit ein Profil die Flags weder umordnen noch doppeln
//!    kann,
//! 2. `--die-with-parent`, `--new-session`, `--cap-drop ALL`,
//!    `--disable-userns`, `--hostname`; die ersten vier stehen immer da, das
//!    Profil kann sie nicht abwählen,
//! 3. `--json-status-fd`, wenn der Launcher eine Status-Pipe mitgibt,
//! 4. `--ro-bind` je `mounts.ro`, dann die `--symlink`,
//! 5. `--proc`, `--dev`, dann die `--tmpfs` außerhalb von `/work`,
//! 6. `mounts.extra_ro` und `mounts.extra_rw`, die Erweiterungen des Nutzers,
//! 7. der Bind des Projektverzeichnisses, dann die `--tmpfs` darunter,
//! 8. Proxy-Socket, CA, das CA-Bundle über [`CA_BUNDLE_DST`], Shim, dann die
//!    drei Identitätsdateien
//!    ([`PASSWD_DST`], [`GROUP_DST`], [`HOSTS_DST`]) und die Maskierungen,
//!    beides als `--ro-bind-data` aus dem Speicher des Launchers
//!    ([`SandboxProfile::masks_to_render`]: die Pflichteinträge zuerst),
//! 9. `--clearenv`, die `--setenv` alphabetisch: das `[env]` des Profils und
//!    die Variablen für den Shim ([`crate::bridge_env`]) in einer Liste,
//! 10. `--chdir`, `--`, der Shim mit `--proxy-port`, `--`, der Befehl.
//!
//! Der Grund für die Reihenfolge: `bwrap` arbeitet die Argumente der Reihe nach
//! ab, ein späterer Mount verdeckt einen früheren. Ein `--tmpfs
//! /work/.git/hooks` vor dem Bind von `/work` wäre danach wieder verdeckt. Und
//! die Erweiterungen des Nutzers stehen vor allem, was die Sitzung einhängt:
//! eine Erweiterung darf weder das Projekt noch die CA verdecken, sonst zeigt
//! die Zeile eine Sandbox, die so nicht läuft. Der Shim und der Proxy-Socket
//! liegen unter `/run/humanitl`, dem einen Verzeichnis, das nie vom Host
//! kommt (`/run` steht auf der Denylist): `bwrap` legt den Mountpoint einer
//! Datei erst beim Einhängen an, und in einem nur lesbaren Bind ginge das
//! nicht (`/usr/local/bin/humanitl-shim` unter dem `--ro-bind /usr` scheitert
//! mit `EROFS`, gemessen mit bubblewrap 0.11).
//!
//! # Was unter `/work` liegt, entscheidet der Host
//!
//! `bwrap` legt den Mountpoint eines `--tmpfs` oder `--ro-bind-data` an, wenn
//! er fehlt: auf einem beschreibbaren `/work` entstünde dann ein leeres
//! `.idea/` im Projekt des Nutzers, auf einem nur lesbaren scheitert der Start
//! mit `EROFS`. Deshalb rendert der Launcher ein `--tmpfs` unter `/work` nur,
//! wenn das Verzeichnis auf dem Host existiert, und eine Maske nur über eine
//! Datei, die es gibt ([`LaunchInputs::present_under_work`]). Was fehlt, muss
//! nicht überdeckt werden.
//!
//! # Ein Deskriptor je Datei
//!
//! `--ro-bind-data FD DST` liest `FD` bis zum Ende und schließt ihn danach;
//! ein zweites `--ro-bind-data` mit derselben Nummer scheitert mit `EBADF`
//! (gemessen mit bubblewrap 0.11). Deshalb trägt [`LaunchInputs`] je Maske
//! und je Identitätsdatei einen eigenen Deskriptor, und der Launcher legt
//! genauso viele memfds an, wie [`SandboxProfile::masks_to_render`] Masken
//! nennt.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};

use humanitl_config::WorkMode;

use crate::bridge_env::shim_env;
use crate::profile::{CA_BUNDLE_DST, Namespace, SandboxProfile, SessionContext};

/// Die Nutzerdatenbank der Sandbox: eine Zeile, der Nutzer der Sitzung.
///
/// Ohne sie kennt `getpwuid(3)` die eigene Kennung nicht, und `id -un`,
/// `git commit` ohne `user.email` oder `os.userInfo()` in Node scheitern.
pub const PASSWD_DST: &str = "/etc/passwd";

/// Die Gruppendatenbank der Sandbox: eine Zeile, die Gruppe des Nutzers.
pub const GROUP_DST: &str = "/etc/group";

/// Die Hosts-Datei der Sandbox: `127.0.0.1 localhost <hostname>`, sonst nichts.
///
/// Es gibt keine `/etc/resolv.conf`; `getaddrinfo(3)` scheitert für jeden
/// anderen Namen sofort, statt zu hängen.
pub const HOSTS_DST: &str = "/etc/hosts";

/// Der Nutzername in der Sandbox, wenn das `[env]` des Profils keinen
/// brauchbaren `USER` nennt.
pub const DEFAULT_USER: &str = "agent";

/// Das Heimatverzeichnis in der Sandbox, wenn das `[env]` des Profils keinen
/// absoluten `HOME` nennt.
pub const DEFAULT_HOME: &str = "/home/agent";

/// Die Login-Shell in `/etc/passwd`.
pub const SANDBOX_SHELL: &str = "/bin/sh";

/// Ab dieser Nummer zählt die Vorschau die Deskriptoren der Masken, eine je
/// Maske, aufsteigend. Die Vorschau belegt 10 bis 14 für Bericht, Status und
/// die drei Identitätsdateien.
pub const PREVIEW_MASK_FD_FIRST: RawFd = 15;

/// Ab dieser Nummer zählt die Vorschau die Deskriptoren der Adapter-Dateien.
///
/// Weit genug hinter den Masken, damit die Zahlen der Vorschau stabil bleiben,
/// wenn ein Profil eine Maske mehr oder weniger trägt. Beim Start vergibt der
/// Launcher die echten Nummern; die Vorschau ist Anzeige, nicht Ausführung.
pub const PREVIEW_AGENT_FD_FIRST: RawFd = 40;

/// Zeichen, die eine Shell unverändert liest und die deshalb ohne
/// Anführungszeichen auskommen.
fn is_safe(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '_' | '@' | '%' | '+' | '=' | ':' | ',' | '.' | '/' | '-')
}

/// Woher die Deskriptoren der Masken kommen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MaskFds {
    /// Die Vorschau: fortlaufende Nummern ab [`PREVIEW_MASK_FD_FIRST`], eine
    /// je Maske. Für Anzeige, Schnappschuss und Tests; nicht startbar.
    Preview,
    /// Ein Start: genau ein Deskriptor je gerenderter Maske, in der
    /// Reihenfolge von [`SandboxProfile::masks_to_render`]. Fehlt einer,
    /// steht `-1` in der Zeile, und `bwrap` startet nicht: lieber kein Start
    /// als eine Maske, die einen fremden Deskriptor liest, oder eine, die
    /// fehlt.
    Each(Vec<RawFd>),
}

impl MaskFds {
    fn get(&self, index: usize) -> RawFd {
        match self {
            Self::Preview => RawFd::try_from(index)
                .ok()
                .and_then(|offset| PREVIEW_MASK_FD_FIRST.checked_add(offset))
                .unwrap_or(-1),
            Self::Each(fds) => fds.get(index).copied().unwrap_or(-1),
        }
    }
}

/// Die Deskriptoren der drei Identitätsdateien, jeder mit dem Inhalt aus
/// [`SandboxProfile::identity_files`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdentityFds {
    /// Für [`PASSWD_DST`].
    pub passwd: RawFd,
    /// Für [`GROUP_DST`].
    pub group: RawFd,
    /// Für [`HOSTS_DST`].
    pub hosts: RawFd,
}

/// Der Inhalt der drei Identitätsdateien, siehe [`SandboxProfile::identity_files`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityFiles {
    /// `<user>:x:<uid>:<gid>:<user>:<home>:/bin/sh`.
    pub passwd: String,
    /// `<user>:x:<gid>:`.
    pub group: String,
    /// `127.0.0.1 localhost <hostname>`.
    pub hosts: String,
}

/// Was nur der Launcher weiß: seine Deskriptoren und der Blick auf den Host.
///
/// Das Profil sagt, was in der Sandbox liegt; der [`SessionContext`] sagt,
/// woher es kommt; hier steht, mit welchen Deskriptoren `bwrap` es bekommt.
/// Die Nummern sind die, unter denen `bwrap` die Deskriptoren erbt: der
/// Launcher legt sie ohne `FD_CLOEXEC` an und schreibt ihre Nummer in die
/// Zeile, es gibt keine Umnummerierung im Kind (die bräuchte `pre_exec`, also
/// `unsafe`, und diese Crate verbietet das).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchInputs {
    /// Die Deskriptoren der Masken (`--ro-bind-data`), einer je Maske.
    pub masks: MaskFds,
    /// Die Deskriptoren der Identitätsdateien; `None` rendert keine.
    pub identity: Option<IdentityFds>,
    /// Die Schreibseite der Berichts-Pipe des Shims, wenn es eine gibt;
    /// wird als [`crate::bridge_env::ENV_REPORT_FD`] gesetzt.
    pub report_fd: Option<RawFd>,
    /// Die Schreibseite der Status-Pipe von `bwrap` (`--json-status-fd`),
    /// wenn es eine gibt.
    pub status_fd: Option<RawFd>,
    /// Die Pfade unter `/work` (Sandbox-Sicht), die auf dem Host existieren.
    /// `None` rendert jeden Eintrag; das ist die Vorschau, nicht der Start.
    pub present_under_work: Option<BTreeSet<PathBuf>>,
    /// Die Dateien des Agent-Adapters, je Eintrag ein Deskriptor und sein Ziel
    /// in der Sandbox, in der Reihenfolge von
    /// [`crate::profile::SessionContext::files`].
    ///
    /// Gerendert wird `--ro-bind-data FD DST`, nach den Masken und vor
    /// `--clearenv`. Leer heißt: kein Argument, und die Zeile sieht aus wie
    /// vorher. `-1` steht für einen Deskriptor, der fehlt; `bwrap` startet dann
    /// nicht, und das ist die richtige Antwort — eine Konfiguration, die der
    /// Agent nicht vorfindet, wäre schlimmer als kein Start.
    pub agent_files: Vec<(RawFd, PathBuf)>,
}

impl LaunchInputs {
    /// Die Vorschau: feste Nummern (Bericht 10, Status 11, Identität 12 bis
    /// 14, Masken ab [`PREVIEW_MASK_FD_FIRST`]) und alles unter `/work` als
    /// vorhanden angenommen. Für Anzeige, Schnappschuss und Tests.
    #[must_use]
    pub const fn preview() -> Self {
        Self {
            masks: MaskFds::Preview,
            identity: Some(IdentityFds {
                passwd: 12,
                group: 13,
                hosts: 14,
            }),
            report_fd: Some(10),
            status_fd: Some(11),
            present_under_work: None,
            agent_files: Vec::new(),
        }
    }

    /// Dieselbe Vorschau, aber mit den Dateien eines Agent-Adapters.
    ///
    /// Die Deskriptoren zählen ab [`PREVIEW_AGENT_FD_FIRST`], einer je Datei,
    /// in der Reihenfolge der Liste. Für `humanitl sandbox argv`: die Zeile
    /// soll zeigen, was startet, und dazu gehören die Dateien des Adapters.
    #[must_use]
    pub fn preview_with_agent_files(dsts: impl IntoIterator<Item = PathBuf>) -> Self {
        let agent_files = dsts
            .into_iter()
            .enumerate()
            .map(|(index, dst)| {
                let fd = RawFd::try_from(index)
                    .ok()
                    .and_then(|offset| PREVIEW_AGENT_FD_FIRST.checked_add(offset))
                    .unwrap_or(-1);
                (fd, dst)
            })
            .collect();
        Self {
            agent_files,
            ..Self::preview()
        }
    }

    fn renders(&self, under_work: &Path) -> bool {
        self.present_under_work
            .as_ref()
            .is_none_or(|present| present.contains(under_work))
    }
}

impl SandboxProfile {
    /// Die vollständige Argumentliste für `bwrap`, ohne `bwrap` selbst.
    ///
    /// Endet mit `--`, dem Shim in der Sandbox, seinem `--proxy-port`, einem
    /// weiteren `--` und dem Befehl aus dem [`SessionContext`].
    #[must_use]
    pub fn to_bwrap_args(&self, ctx: &SessionContext, inputs: &LaunchInputs) -> Vec<OsString> {
        let mut args = Args::default();

        // Die feste Reihenfolge, nicht die des Profils: `parse` verlangt, dass
        // alle sechs genannt sind, also ist die Liste hier vollständig.
        for namespace in Namespace::ALL {
            args.flag(namespace.flag());
        }
        // Nicht abschaltbar, auch nicht über die Felder des Profils: eine
        // Sandbox, die den Daemon überlebt oder ins Terminal des Nutzers
        // schreibt, ist keine, und eine mit Capabilities auch nicht
        // (`backlog/CONVENTIONS.md` 4.10, 4.11). `--disable-userns` nimmt dem
        // Agenten den zweiten Nutzer-Namensraum, in dem er sich selbst wieder
        // Capabilities geben könnte; ab bwrap 0.6, das Profil verlangt 0.8.
        args.flag("--die-with-parent");
        args.flag("--new-session");
        args.pair("--cap-drop", "ALL");
        args.flag("--disable-userns");
        args.pair("--hostname", &self.sandbox.hostname);
        if let Some(fd) = inputs.status_fd {
            args.pair("--json-status-fd", fd.to_string());
        }

        for path in &self.mounts.ro {
            args.bind("--ro-bind", path, path);
        }
        for link in &self.mounts.symlinks {
            args.bind("--symlink", Path::new(&link.target), &link.link);
        }
        if let Some(proc) = self.mounts.proc.as_deref() {
            args.path("--proc", proc);
        }
        if let Some(dev) = self.mounts.dev.as_deref() {
            args.path("--dev", dev);
        }

        let work_dst = self.mounts.work.dst.as_path();
        let (under_work, elsewhere): (Vec<_>, Vec<_>) = self
            .mounts
            .tmpfs
            .iter()
            .partition(|path| path.starts_with(work_dst) && path.as_path() != work_dst);
        for path in elsewhere {
            args.path("--tmpfs", path);
        }

        // Die Erweiterungen des Nutzers kommen vor dem, was die Sitzung
        // einhängt: was danach kommt, liegt obenauf und kann nicht verdeckt
        // werden.
        for path in &self.mounts.extra_ro {
            args.bind("--ro-bind", path, path);
        }
        for path in &self.mounts.extra_rw {
            args.bind("--bind", path, path);
        }

        let work_flag = match self.effective_work_mode(ctx.work_mode) {
            WorkMode::Ro => "--ro-bind",
            WorkMode::Rw => "--bind",
        };
        args.bind(work_flag, &ctx.work_src, work_dst);
        for path in under_work {
            if inputs.renders(path) {
                args.path("--tmpfs", path);
            }
        }

        // Nur lesbar eingehängt, auch der Socket: `connect(2)` bleibt
        // erlaubt, weil der Kernel `EROFS` nur beim Öffnen und Ändern von
        // Dateien liefert, `chmod(2)` auf die Socket-Datei des Hosts dagegen
        // nicht (mit `--bind` könnte der Agent sie auf 0666 stellen; gemessen
        // mit bubblewrap 0.11). Die Datei, nie ihr Verzeichnis: neben dem
        // Proxy-Socket läge sonst, was der Daemon noch so hat (HUM-013).
        args.bind(
            "--ro-bind",
            &ctx.proxy_socket_src,
            &self.network.proxy_socket_dst,
        );
        args.bind("--ro-bind", &ctx.ca_cert_src, &self.network.ca_cert_dst);
        // Die Überdeckung des System-Bundles: dieselben Wurzeln, die der Host
        // hat, plus die eigene CA (HUM-014). Sie steht hier und nicht bei
        // `mounts.ro`, weil `bwrap` die Argumente der Reihe nach abarbeitet
        // und der `--ro-bind /etc/ssl` des Profils sie sonst wieder verdeckte.
        // Ohne sie lehnt jeder TLS-Client in der Sandbox das Leaf des Proxys
        // ab, und `SSL_CERT_FILE` allein erreicht nicht jedes Werkzeug
        // ([`CA_BUNDLE_DST`]).
        args.bind("--ro-bind", &ctx.ca_bundle_src, Path::new(CA_BUNDLE_DST));
        args.bind("--ro-bind", &ctx.shim_src, &self.network.shim_dst);

        // Wer der Nutzer ist, sagt der Launcher, nicht der Host: kein
        // Host-`/etc/passwd`, nur die eine Zeile für diese Kennung.
        if let Some(identity) = &inputs.identity {
            args.data(identity.passwd, Path::new(PASSWD_DST));
            args.data(identity.group, Path::new(GROUP_DST));
            args.data(identity.hosts, Path::new(HOSTS_DST));
        }

        // Eine leere, nur lesbare Datei über der Quelle, Inhalt aus dem memfd
        // des Launchers. Nicht `/dev/null`: der Bind eines Gerätes auf einem
        // `nodev`-Mount antwortet `EACCES`, und `git` in der Sandbox soll
        // `.git/config` lesen können, nur eben leer. Die Pflichteinträge
        // kommen immer, auch bei `masked_files = []`.
        for (index, path) in self
            .masks_to_render(inputs.present_under_work.as_ref())
            .iter()
            .enumerate()
        {
            args.data(inputs.masks.get(index), path);
        }

        // Die Dateien des Agent-Adapters, jede aus ihrem eigenen memfd. Sie
        // stehen nach den Masken, damit ein Profil, das denselben Ort
        // maskiert, nicht die Konfiguration verdeckt, und vor `--clearenv`,
        // weil `bwrap` seine Argumente der Reihe nach abarbeitet.
        for (fd, dst) in &inputs.agent_files {
            args.data(*fd, dst);
        }

        args.flag("--clearenv");
        for (key, value) in self.effective_env(ctx, inputs.report_fd) {
            args.setenv(&key, &value);
        }

        args.path("--chdir", work_dst);
        args.flag("--");
        args.push(&self.network.shim_dst);
        args.pair("--proxy-port", self.network.proxy_port.to_string());
        args.flag("--");
        for part in &ctx.command {
            args.push(part);
        }

        args.0
    }

    /// Dieselbe Liste als eine Zeile, wie eine Shell sie lesen würde.
    ///
    /// Für die Anzeige und für `humanitl sandbox argv`. Zitiert wird nach POSIX
    /// mit einfachen Anführungszeichen; `shlex::split` der Ausgabe ergibt wieder
    /// genau [`SandboxProfile::to_bwrap_args`].
    ///
    /// Das Programm selbst steht nicht in der Zeile, so wie es auch nicht in der
    /// Liste steht: welche `bwrap`-Datei gestartet wird, entscheidet der
    /// Launcher beim Auflösen über `PATH`; [`crate::LaunchPlan::argv_line`]
    /// zeigt die Zeile mit Programm.
    ///
    /// Ein Argument, das kein gültiges UTF-8 ist, erscheint in der Zeile ersetzt
    /// (`to_string_lossy`). Die Zeile ist Anzeige, nicht Ausführung: gestartet
    /// wird immer die Liste.
    #[must_use]
    pub fn argv_line(&self, ctx: &SessionContext, inputs: &LaunchInputs) -> String {
        shell_line(&self.to_bwrap_args(ctx, inputs))
    }

    /// Die Masken, die [`SandboxProfile::to_bwrap_args`] mit diesem Blick auf
    /// den Host rendert, in dieser Reihenfolge: die Pflichteinträge zuerst,
    /// dann `mounts.masked_files`, jeweils nur, was unter `/work` vorhanden
    /// ist (`None`: alles). Der Launcher legt je Eintrag einen Deskriptor an.
    #[must_use]
    pub fn masks_to_render(&self, present_under_work: Option<&BTreeSet<PathBuf>>) -> Vec<PathBuf> {
        self.effective_masked_files()
            .into_iter()
            .filter(|path| present_under_work.is_none_or(|present| present.contains(path)))
            .collect()
    }

    /// Der Nutzername in der Sandbox: `USER` aus dem `[env]` des Profils,
    /// wenn er ein schlichter Name ist (Buchstaben, Ziffern, `_`, `-`, `.`),
    /// sonst [`DEFAULT_USER`].
    #[must_use]
    pub fn user_name(&self) -> &str {
        self.env
            .get("USER")
            .map(String::as_str)
            .filter(|name| {
                !name.is_empty()
                    && name.len() <= 32
                    && name
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
            })
            .unwrap_or(DEFAULT_USER)
    }

    /// Das Heimatverzeichnis in der Sandbox: `HOME` aus dem `[env]` des
    /// Profils, wenn es absolut ist, sonst [`DEFAULT_HOME`].
    #[must_use]
    pub fn home_dir(&self) -> &str {
        self.env
            .get("HOME")
            .map(String::as_str)
            .filter(|home| Path::new(home).is_absolute() && !home.contains(':'))
            .unwrap_or(DEFAULT_HOME)
    }

    /// Der Inhalt von `/etc/passwd`, `/etc/group` und `/etc/hosts` für den
    /// Nutzer mit dieser Kennung.
    ///
    /// `bwrap` bildet ohne `--uid` die Kennung des Aufrufers auf dieselbe
    /// Nummer ab; deshalb sind `uid` und `gid` die des Launchers, der Name
    /// aber der aus dem Profil ([`SandboxProfile::user_name`]). Der Host
    /// und seine Nutzerliste bleiben draußen.
    #[must_use]
    pub fn identity_files(&self, uid: u32, gid: u32) -> IdentityFiles {
        let user = self.user_name();
        let home = self.home_dir();
        IdentityFiles {
            passwd: format!("{user}:x:{uid}:{gid}:{user}:{home}:{SANDBOX_SHELL}\n"),
            group: format!("{user}:x:{gid}:\n"),
            hosts: format!("127.0.0.1 localhost {}\n", self.sandbox.hostname),
        }
    }

    /// Die Umgebung, die das Profil in der Sandbox setzt.
    ///
    /// Nur das `[env]` des Profils; die Variablen für den Shim kommen mit
    /// [`SandboxProfile::effective_env`] dazu.
    #[must_use]
    pub fn env_pairs(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    /// Die vollständige Umgebung, die `--setenv` aufbaut, alphabetisch: das
    /// `[env]` des Profils, darüber die Variablen der Sitzung
    /// ([`SessionContext::session_env`], üblicherweise
    /// `humanitl_proxy::ca::env_kit` mit `HUMANITL_SESSION`), darüber die
    /// Variablen für den Shim aus [`crate::bridge_env`].
    ///
    /// Die Rangfolge ist damit: Profil < Sitzung < Shim. Das Profil beschreibt
    /// den Normalfall, die Sitzung weiß, welche Sitzung läuft und wo ihre CA
    /// liegt, und die fünf Variablen des Shims sind eine
    /// Sicherheitsentscheidung, die weder Profil noch Sitzung überschreiben
    /// können ([`crate::bridge_env::RESERVED_ENV`]). Der Shim nimmt seine fünf
    /// Variablen vor `exec` wieder heraus; der Agent sieht den Rest. Für
    /// `--setenv` und für [`crate::LaunchPlan::env`].
    #[must_use]
    pub fn effective_env(
        &self,
        session: &SessionContext,
        report_fd: Option<RawFd>,
    ) -> Vec<(String, String)> {
        let mut merged = self.env.clone();
        for (key, value) in &session.session_env {
            merged.insert(key.clone(), value.clone());
        }
        for (key, value) in shim_env(self, report_fd) {
            merged.insert(key, value);
        }
        merged.into_iter().collect()
    }
}

/// Die Argumente als eine Zeile, jedes nach POSIX zitiert.
#[must_use]
pub fn shell_line(args: &[OsString]) -> String {
    args.iter()
        .map(|arg| shell_quote(&arg.to_string_lossy()))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Setzt ein Argument so in einfache Anführungszeichen, dass `sh` es wieder
/// als genau ein Wort liest.
#[must_use]
pub fn shell_quote(arg: &str) -> String {
    if !arg.is_empty() && arg.chars().all(is_safe) {
        return arg.to_owned();
    }
    let mut out = String::with_capacity(arg.len() + 2);
    out.push('\'');
    for c in arg.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Sammelt die Argumente und hält die Schreibweise an einer Stelle.
#[derive(Debug, Default)]
struct Args(Vec<OsString>);

impl Args {
    fn push(&mut self, value: impl AsRef<OsStr>) {
        self.0.push(value.as_ref().to_os_string());
    }

    fn flag(&mut self, flag: &str) {
        self.push(flag);
    }

    fn pair(&mut self, flag: &str, value: impl AsRef<OsStr>) {
        self.push(flag);
        self.push(value);
    }

    fn path(&mut self, flag: &str, path: &Path) {
        self.push(flag);
        self.push(path);
    }

    fn bind(&mut self, flag: &str, src: &Path, dst: &Path) {
        self.push(flag);
        self.push(src);
        self.push(dst);
    }

    /// `--ro-bind-data FD DST`: eine Datei aus einem Deskriptor des Launchers.
    fn data(&mut self, fd: RawFd, dst: &Path) {
        self.push("--ro-bind-data");
        self.push(fd.to_string());
        self.push(dst);
    }

    fn setenv(&mut self, key: &str, value: &str) {
        self.push("--setenv");
        self.push(key);
        self.push(value);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use humanitl_config::WorkMode;
    use humanitl_core::ids::SessionId;

    use super::{IdentityFds, LaunchInputs, MaskFds, PREVIEW_MASK_FD_FIRST, shell_quote};
    use crate::profile::{SandboxProfile, SessionContext};

    #[test]
    fn quoting_leaves_ordinary_words_alone() {
        assert_eq!(shell_quote("--ro-bind"), "--ro-bind");
        assert_eq!(shell_quote("/etc/ssl"), "/etc/ssl");
        assert_eq!(
            shell_quote("http://127.0.0.1:3128"),
            "http://127.0.0.1:3128"
        );
    }

    #[test]
    fn quoting_wraps_what_a_shell_would_read_differently() {
        assert_eq!(shell_quote(""), "''");
        assert_eq!(shell_quote("echo hello"), "'echo hello'");
        assert_eq!(shell_quote("a*b"), "'a*b'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
    }

    fn context() -> SessionContext {
        SessionContext {
            session: SessionId::nil(),
            work_src: PathBuf::from("/home/u/proj"),
            work_mode: WorkMode::Rw,
            proxy_socket_src: PathBuf::from("/run/user/1000/humanitl/proxy/proxy.sock"),
            ca_cert_src: PathBuf::from("/home/u/.local/share/humanitl/ca/ca.crt"),
            ca_bundle_src: PathBuf::from("/home/u/.local/share/humanitl/ca/ca-bundle.crt"),
            shim_src: PathBuf::from("/usr/lib/humanitl/humanitl-shim"),
            session_env: Vec::new(),
            command: vec!["true".into()],
            files: Vec::new(),
        }
    }

    fn strings(profile: &SandboxProfile, inputs: &LaunchInputs) -> Vec<String> {
        profile
            .to_bwrap_args(&context(), inputs)
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn window_at(args: &[String], window: &[&str]) -> Option<usize> {
        args.windows(window.len())
            .position(|w| w.iter().zip(window).all(|(a, b)| a == b))
    }

    /// Nur was auf dem Host existiert, wird unter `/work` überdeckt.
    #[test]
    fn masks_and_tmpfs_under_work_follow_the_host() {
        let profile = SandboxProfile::parse(
            "version = 1\nname = \"x\"\n[mounts]\ntmpfs = [\"/tmp\", \"/dev/shm\", \"/work/.git/hooks\", \"/work/.idea\"]\n",
            Path::new("<t>"),
        )
        .expect("parses");

        let everything = strings(&profile, &LaunchInputs::preview());
        for expected in [
            "/work/.git/hooks",
            "/work/.idea",
            "/work/.envrc",
            "/work/.git/config",
        ] {
            assert!(everything.contains(&expected.to_owned()), "{expected}");
        }

        let present: BTreeSet<PathBuf> = ["/work/.git/hooks", "/work/.git/config"]
            .map(PathBuf::from)
            .into_iter()
            .collect();
        assert_eq!(
            profile.masks_to_render(Some(&present)),
            [PathBuf::from("/work/.git/config")]
        );
        assert_eq!(
            profile.masks_to_render(None),
            ["/work/.envrc", "/work/.git/config"].map(PathBuf::from)
        );
        let some = strings(
            &profile,
            &LaunchInputs {
                present_under_work: Some(present),
                ..LaunchInputs::preview()
            },
        );
        assert!(some.contains(&"/work/.git/hooks".to_owned()));
        assert!(some.contains(&"/work/.git/config".to_owned()));
        assert!(!some.contains(&"/work/.idea".to_owned()));
        assert!(!some.contains(&"/work/.envrc".to_owned()));
        // Die tmpfs außerhalb von /work werden nie gefiltert.
        assert!(some.contains(&"/tmp".to_owned()));
        assert!(some.contains(&"/dev/shm".to_owned()));
    }

    /// Ohne Pipes fehlen `--json-status-fd` und `HUMANITL_REPORT_FD`, ohne
    /// Identität die drei `/etc`-Dateien, sonst nichts.
    #[test]
    fn pipes_and_identity_are_optional_and_leave_no_trace_when_absent() {
        let profile =
            SandboxProfile::parse("version = 1\nname = \"x\"\n", Path::new("<t>")).expect("parses");
        let with = strings(&profile, &LaunchInputs::preview());
        let without = strings(
            &profile,
            &LaunchInputs {
                masks: MaskFds::Preview,
                identity: None,
                report_fd: None,
                status_fd: None,
                present_under_work: None,
                agent_files: Vec::new(),
            },
        );
        assert!(with.windows(2).any(|w| w == ["--json-status-fd", "11"]));
        assert!(
            with.windows(3)
                .any(|w| w == ["--setenv", "HUMANITL_REPORT_FD", "10"])
        );
        assert!(window_at(&with, &["--ro-bind-data", "12", "/etc/passwd"]).is_some());
        assert!(window_at(&with, &["--ro-bind-data", "13", "/etc/group"]).is_some());
        assert!(window_at(&with, &["--ro-bind-data", "14", "/etc/hosts"]).is_some());
        assert!(!without.contains(&"--json-status-fd".to_owned()));
        assert!(!without.contains(&"HUMANITL_REPORT_FD".to_owned()));
        assert!(!without.contains(&"/etc/passwd".to_owned()));
        assert_eq!(with.len(), without.len() + 5 + 9);

        // Eine Maske, ein Deskriptor: die Vorschau zählt ab PREVIEW_MASK_FD_FIRST.
        let first = PREVIEW_MASK_FD_FIRST.to_string();
        let second = (PREVIEW_MASK_FD_FIRST + 1).to_string();
        assert!(window_at(&with, &["--ro-bind-data", &first, "/work/.envrc"]).is_some());
        assert!(window_at(&with, &["--ro-bind-data", &second, "/work/.git/config"]).is_some());
    }

    /// Die Dateien des Adapters stehen als `--ro-bind-data` in der Zeile, nach
    /// den Masken und vor `--clearenv`. Ohne Dateien ändert sich nichts.
    #[test]
    fn agent_files_are_rendered_after_the_masks_and_before_clearenv() {
        let profile =
            SandboxProfile::parse("version = 1\nname = \"x\"\n", Path::new("<t>")).expect("parses");
        let plain = strings(&profile, &LaunchInputs::preview());
        let inputs = LaunchInputs {
            agent_files: vec![
                (40, PathBuf::from("/etc/humanitl/opencode/opencode.json")),
                (41, PathBuf::from("/etc/humanitl/opencode/models.json")),
            ],
            ..LaunchInputs::preview()
        };
        let with = strings(&profile, &inputs);

        assert_eq!(with.len(), plain.len() + 6, "two files, three words each");
        let config = window_at(
            &with,
            &[
                "--ro-bind-data",
                "40",
                "/etc/humanitl/opencode/opencode.json",
            ],
        )
        .expect("the config is in the line");
        let models = window_at(
            &with,
            &["--ro-bind-data", "41", "/etc/humanitl/opencode/models.json"],
        )
        .expect("the catalog is in the line");
        let last_mask = window_at(
            &with,
            &[
                "--ro-bind-data",
                &(PREVIEW_MASK_FD_FIRST + 1).to_string(),
                "/work/.git/config",
            ],
        )
        .expect("the masks are in the line");
        let clearenv = with
            .iter()
            .position(|arg| arg == "--clearenv")
            .expect("--clearenv is in the line");
        assert!(last_mask < config, "adapter files come after the masks");
        assert!(config < models, "the order is the order of the list");
        assert!(models < clearenv, "adapter files come before --clearenv");
    }

    /// Beim Start trägt jede Maske ihren eigenen Deskriptor; fehlt einer,
    /// steht `-1` da, und `bwrap` verweigert den Start, statt einen fremden
    /// Deskriptor zu lesen.
    #[test]
    fn each_mask_takes_its_own_descriptor_and_a_missing_one_is_minus_one() {
        let profile =
            SandboxProfile::parse("version = 1\nname = \"x\"\n", Path::new("<t>")).expect("parses");
        let args = strings(
            &profile,
            &LaunchInputs {
                masks: MaskFds::Each(vec![40]),
                identity: Some(IdentityFds {
                    passwd: 30,
                    group: 31,
                    hosts: 32,
                }),
                report_fd: None,
                status_fd: None,
                present_under_work: None,
                agent_files: Vec::new(),
            },
        );
        assert!(window_at(&args, &["--ro-bind-data", "40", "/work/.envrc"]).is_some());
        assert!(window_at(&args, &["--ro-bind-data", "-1", "/work/.git/config"]).is_some());
        assert!(window_at(&args, &["--ro-bind-data", "30", "/etc/passwd"]).is_some());
        assert!(window_at(&args, &["--ro-bind-data", "32", "/etc/hosts"]).is_some());
    }

    /// Weder Profil noch Sitzung können die Variablen des Shims
    /// überschreiben, und die Sitzung gewinnt über das Profil.
    #[test]
    fn the_shim_variables_win_over_the_profile_and_the_session() {
        let profile = SandboxProfile::parse(
            "version = 1\nname = \"x\"\n[env]\nHUMANITL_SECCOMP_FAMILIES = \"AF_UNIX,AF_PACKET\"\nHTTP_PROXY = \"http://profile\"\nZZZ = \"1\"\n",
            Path::new("<t>"),
        )
        .expect("parses");
        let mut ctx = context();
        ctx.session_env = vec![
            ("HUMANITL_SESSION".to_owned(), "0198-abc".to_owned()),
            ("HTTP_PROXY".to_owned(), "http://127.0.0.1:3128".to_owned()),
            (
                "HUMANITL_SECCOMP_FAMILIES".to_owned(),
                "AF_PACKET".to_owned(),
            ),
        ];
        let env = profile.effective_env(&ctx, None);
        let get = |key: &str| {
            env.iter()
                .find(|(k, _)| k == key)
                .map_or_else(|| panic!("{key} missing"), |(_, v)| v.as_str())
        };
        // Der Shim gewinnt über beide.
        assert_eq!(get("HUMANITL_SECCOMP_FAMILIES"), "AF_INET,AF_INET6");
        // Die Sitzung gewinnt über das Profil.
        assert_eq!(get("HTTP_PROXY"), "http://127.0.0.1:3128");
        // Und was nur die Sitzung kennt, kommt an.
        assert_eq!(get("HUMANITL_SESSION"), "0198-abc");
        let keys: Vec<&str> = env.iter().map(|(k, _)| k.as_str()).collect();
        let mut sorted = keys.clone();
        sorted.sort_unstable();
        assert_eq!(keys, sorted, "alphabetical, shim variables included");
        assert_eq!(keys.last(), Some(&"ZZZ"));
    }

    /// Die Identität kommt aus dem Profil und der Kennung des Launchers, nie
    /// vom Host: ein `USER`, der kein Name ist, fällt auf `agent` zurück.
    #[test]
    fn identity_files_name_the_profile_user_with_the_launchers_ids() {
        let profile =
            SandboxProfile::parse("version = 1\nname = \"x\"\n", Path::new("<t>")).expect("parses");
        let files = profile.identity_files(1000, 1000);
        assert_eq!(
            files.passwd,
            "agent:x:1000:1000:agent:/home/agent:/bin/sh\n"
        );
        assert_eq!(files.group, "agent:x:1000:\n");
        assert_eq!(files.hosts, "127.0.0.1 localhost sandbox\n");

        let profile = SandboxProfile::parse(
            "version = 1\nname = \"x\"\n[env]\nUSER = \"dev.1\"\nHOME = \"/srv/dev\"\n",
            Path::new("<t>"),
        )
        .expect("parses");
        let files = profile.identity_files(1001, 100);
        assert_eq!(files.passwd, "dev.1:x:1001:100:dev.1:/srv/dev:/bin/sh\n");
        assert_eq!(files.group, "dev.1:x:100:\n");

        let profile = SandboxProfile::parse(
            "version = 1\nname = \"x\"\n[env]\nUSER = \"root:x:0\"\nHOME = \"relative\"\n",
            Path::new("<t>"),
        )
        .expect("parses");
        assert_eq!(profile.user_name(), "agent");
        assert_eq!(profile.home_dir(), "/home/agent");
        assert!(!profile.identity_files(1, 1).passwd.contains("root"));
    }
}
