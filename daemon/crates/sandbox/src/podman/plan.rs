//! Vom Profil zur Zeile `podman run` (HUM-433).
//!
//! Alles hier ist rein: Aus Profil, Sitzung, den Quellen und der Lage der
//! Deskriptoren entstehen die Einhängungen samt ihrer Tabelle
//! ([`mounts`]), das Gerüst der leeren Wurzel ([`skeleton`]) und die Zeile
//! ([`line`]). Der Plan in `mod.rs` füllt die Quellen mit geprüften Pfaden
//! und Dateien, die Vorschau mit den Pfaden der Sitzung; beide bauen die
//! Zeile mit denselben Funktionen.
//!
//! # Die festen Flags
//!
//! [`FIXED_RUN_FLAGS`] steht in jeder Zeile genau einmal, gleich welches
//! Profil: `--rm`, eigene Namensräume für Netz, IPC, UTS, PID und cgroup,
//! `--userns keep-id`, keine Capabilities, `no-new-privileges`, eine nur
//! lesbare Wurzel ohne die tmpfs, die Podman zu `--read-only` sonst auf
//! `/run`, `/tmp` und `/var/tmp` legte, kein Protokoll, kein Pull, keine
//! `/etc/hosts` und keine Benutzerdatenbank von Podman, keine Umgebung und
//! keine Proxy-Variablen des Hosts. Was sie aufweichen könnte, hat keinen
//! Schlüssel: kein `-e`, kein `-t`, kein `--privileged`, kein anderes Netz,
//! kein `--cap-add`, kein `--device`, kein Umetikettieren (`:z`, `:Z`);
//! [`FORBIDDEN`] hält das als Liste fest, und die Tests prüfen jede Zeile
//! dagegen.
//!
//! # Einhängen über Pfade
//!
//! Podman nimmt keinen Deskriptor als Quelle (HUM-292 P13: `crun: mount
//! /proc/self/fd/3 to work: Invalid argument`; mit der Zeile des Adapters
//! nachgemessen in HUM-437: kein Container, kein Bericht des Shims). Der Plan öffnet und prüft
//! jede Quelle trotzdem einmal (HUM-263) und hängt den Ort ein, an dem das
//! Geöffnete liegt; dass danach dort noch derselbe Ordner liegt, prüft der
//! Shim nach dem Einhängen über Gerät und Inode (HUM-437). Masken,
//! Identitätsdateien und Dateien des Adapters liegen als Dateien im
//! Verzeichnis der Sitzung, nicht in memfds.
//!
//! Ein Pfad mit Komma, Anführungszeichen oder Zeilenumbruch lässt sich in
//! `--mount` nicht eindeutig schreiben (die Felder trennt ein Komma); der
//! Plan lehnt ihn ab, statt ihn zu zitieren ([`check_expressible`]).

use std::collections::BTreeSet;
use std::ffi::OsString;
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};

use humanitl_config::WorkMode;
use humanitl_core::diagnostics::codes::SANDBOX_006;
use humanitl_core::{Diagnostic, Severity};

use crate::bwrap_args::{GROUP_DST, HOSTS_DST, PASSWD_DST};
use crate::launcher::{PolicyMode, PolicyOrigin, PolicyRow, StdioMode};
use crate::profile::{CA_BUNDLE_DST, SandboxProfile, SessionContext};

/// Die Flags von `podman run`, die jede Zeile genau einmal trägt, mit Wert,
/// wo sie einen haben. Die Werte der übrigen (`--name`, `--memory`, ...)
/// hängen von der Sitzung ab.
pub(crate) const FIXED_RUN_FLAGS: &[&str] = &[
    "--rm",
    "--network=none",
    "--ipc=private",
    "--uts=private",
    "--pid=private",
    "--cgroupns=private",
    "--userns=keep-id",
    "--cap-drop=all",
    "--security-opt=no-new-privileges",
    "--read-only",
    "--read-only-tmpfs=false",
    "--log-driver=none",
    "--pull=never",
    "--no-hosts",
    "--passwd=false",
    "--env-host=false",
    "--http-proxy=false",
];

/// Was keine Zeile tragen darf, als Anfang eines Arguments. `--network`
/// steht nur als `--network=none` darin ([`FIXED_RUN_FLAGS`]).
#[cfg(test)]
pub(crate) const FORBIDDEN: &[&str] = &[
    "-e",
    "--env=",
    "--env-file",
    "--env-merge",
    "-t",
    "--tty",
    "--privileged",
    "--cap-add",
    "--device",
    "--volume",
    "-v",
    "--net=",
    "--security-opt=label",
    "--security-opt=seccomp=unconfined",
    "--userns=host",
    "--pid=host",
    "--ipc=host",
    "--uts=host",
    "--group-add",
    "--secret",
    "--init",
    "--hooks-dir=/usr",
    "--hooks-dir=/etc",
];

/// Die Rechte jedes tmpfs: wie `/tmp`, damit der Agent mit der Kennung des
/// Nutzers hineinschreiben kann.
const TMPFS_MODE: &str = "1777";

/// Die Nummern, unter denen der Container die Deskriptoren des Plans
/// findet: fortlaufend ab 3, in dieser Reihenfolge (`--preserve-fds`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Fds {
    /// Die Schreibseite des Berichts.
    pub(crate) report: RawFd,
    /// Das Leseende der Lebensleine.
    pub(crate) lifeline: RawFd,
    /// Der memfd mit der Umgebung des Agenten.
    pub(crate) env: RawFd,
    /// Der memfd mit der erwarteten Mounttabelle und den geprüften Ordnern
    /// (HUM-437).
    pub(crate) pins: RawFd,
    /// Die Seite des Shims an der Tür zu weiteren Shells, nur am eigenen
    /// Pseudoterminal.
    pub(crate) control: Option<RawFd>,
    /// Die Untertanenseite des Pseudoterminals, als letzte, nur am eigenen
    /// Pseudoterminal.
    pub(crate) stdio: Option<RawFd>,
}

impl Fds {
    /// Die Lage für einen Start mit oder ohne eigenes Pseudoterminal.
    pub(crate) const fn layout(terminal: bool) -> Self {
        if terminal {
            Self {
                report: 3,
                lifeline: 4,
                env: 5,
                pins: 6,
                control: Some(7),
                stdio: Some(8),
            }
        } else {
            Self {
                report: 3,
                lifeline: 4,
                env: 5,
                pins: 6,
                control: None,
                stdio: None,
            }
        }
    }

    /// Wie viele Deskriptoren der Container erbt (`--preserve-fds`).
    pub(crate) fn count(&self) -> usize {
        self.planned() + usize::from(self.stdio.is_some())
    }

    /// Wie viele davon der Plan selbst trägt; die Untertanenseite legt erst
    /// der Start dazu.
    pub(crate) fn planned(&self) -> usize {
        4 + usize::from(self.control.is_some())
    }
}

/// Die Dateien einer Sitzung, die der Plan schreibt und einhängt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionFiles {
    /// `/etc/passwd`.
    pub(crate) passwd: PathBuf,
    /// `/etc/group`.
    pub(crate) group: PathBuf,
    /// `/etc/hosts`.
    pub(crate) hosts: PathBuf,
    /// Eine leere Datei, über jeder Maske.
    pub(crate) mask: PathBuf,
    /// Je Datei des Adapters eine, in der Reihenfolge der Sitzung.
    pub(crate) agent: Vec<PathBuf>,
}

impl SessionFiles {
    /// Die Namen unter `dir`, für `count` Dateien des Adapters.
    pub(crate) fn under(dir: &Path, count: usize) -> Self {
        Self {
            passwd: dir.join("passwd"),
            group: dir.join("group"),
            hosts: dir.join("hosts"),
            mask: dir.join("mask"),
            agent: (0..count)
                .map(|index| dir.join(format!("agent-{index}")))
                .collect(),
        }
    }
}

/// Woher die Einhängungen kommen: beim Start die geprüften Orte, in der
/// Vorschau die Pfade der Sitzung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Sources {
    /// Der Projektordner.
    pub(crate) work: PathBuf,
    /// `mounts.extra_ro`, je Eintrag in dessen Reihenfolge.
    pub(crate) extra_ro: Vec<PathBuf>,
    /// `mounts.extra_rw`, ebenso.
    pub(crate) extra_rw: Vec<PathBuf>,
    /// Die nur lesbaren Binds des Aufrufers, `(Quelle, Ziel)`.
    pub(crate) session_ro: Vec<(PathBuf, PathBuf)>,
    /// Das Programm des Agenten (HUM-278).
    pub(crate) agent_program: Option<PathBuf>,
    /// Die Dateien der Sitzung.
    pub(crate) files: SessionFiles,
}

/// Wie ein Pfad in der Sandbox entsteht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MountKind {
    /// Ein Bind von `source`.
    Bind {
        /// Die Quelle auf dem Host.
        source: PathBuf,
        /// Schreibbar oder nur lesbar.
        writable: bool,
        /// Ob die Quelle ein Verzeichnis ist; bestimmt das Gerüst.
        dir: bool,
    },
    /// Ein leeres tmpfs.
    Tmpfs,
    /// Ein leeres, nur lesbares tmpfs über einem Pfad, den die Engine selbst
    /// einhängt und den das Profil nicht vorsieht (HUM-453: `/sys`).
    Hidden,
}

/// Eine Einhängung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Mount {
    /// Wie.
    pub(crate) kind: MountKind,
    /// Wohin, in der Sandbox.
    pub(crate) dst: PathBuf,
}

impl Mount {
    /// Das Argument von `--mount`.
    pub(crate) fn render(&self) -> String {
        match &self.kind {
            MountKind::Bind {
                source, writable, ..
            } => {
                let mut text = format!(
                    "type=bind,source={},destination={}",
                    source.display(),
                    self.dst.display()
                );
                if !writable {
                    text.push_str(",readonly=true");
                }
                text
            }
            MountKind::Tmpfs => format!(
                "type=tmpfs,destination={},tmpfs-mode={TMPFS_MODE},notmpcopyup",
                self.dst.display()
            ),
            MountKind::Hidden => format!(
                "type=tmpfs,destination={},ro,tmpfs-mode=0555,notmpcopyup",
                self.dst.display()
            ),
        }
    }
}

/// Sammelt Einhängungen und ihre Tabelle an einer Stelle.
struct Collector<'a> {
    mounts: Vec<Mount>,
    rows: Vec<PolicyRow>,
    profile: &'a SandboxProfile,
    session: &'a SessionContext,
}

impl Collector<'_> {
    fn origin(&self, target: &Path) -> PolicyOrigin {
        let network = &self.profile.network;
        if self.session.files.iter().any(|file| file.dst == target) {
            PolicyOrigin::Adapter
        } else if target == self.profile.mounts.work.dst
            || target == network.proxy_socket_dst
            || target == network.ca_cert_dst
            || target == network.shim_dst
            || target == Path::new(CA_BUNDLE_DST)
        {
            PolicyOrigin::Session
        } else if self
            .profile
            .mounts
            .extra_ro
            .iter()
            .chain(&self.profile.mounts.extra_rw)
            .any(|path| path == target)
        {
            PolicyOrigin::User
        } else {
            PolicyOrigin::Profile
        }
    }

    fn bind(&mut self, source: &Path, dst: &Path, writable: bool, dir: bool) {
        let mode = if writable {
            PolicyMode::ReadWrite
        } else {
            PolicyMode::ReadOnly
        };
        let origin = self.origin(dst);
        self.rows.push(PolicyRow::new(
            mode,
            Some(source.to_path_buf()),
            dst.to_path_buf(),
            origin,
        ));
        self.mounts.push(Mount {
            kind: MountKind::Bind {
                source: source.to_path_buf(),
                writable,
                dir,
            },
            dst: dst.to_path_buf(),
        });
    }

    /// Eine Datei aus dem Verzeichnis der Sitzung über `dst`; in der Tabelle
    /// wie unter bwrap eine Maske ohne Quelle.
    fn file(&mut self, source: &Path, dst: &Path) {
        let origin = self.origin(dst);
        self.rows.push(PolicyRow::new(
            PolicyMode::Masked,
            None,
            dst.to_path_buf(),
            origin,
        ));
        self.mounts.push(Mount {
            kind: MountKind::Bind {
                source: source.to_path_buf(),
                writable: false,
                dir: false,
            },
            dst: dst.to_path_buf(),
        });
    }

    /// Verdeckt einen Pfad, den die Engine von sich aus einhängt, mit einem
    /// leeren, nur lesbaren tmpfs.
    fn hide(&mut self, dst: &Path) {
        let origin = self.origin(dst);
        self.rows.push(PolicyRow::new(
            PolicyMode::Masked,
            None,
            dst.to_path_buf(),
            origin,
        ));
        self.mounts.push(Mount {
            kind: MountKind::Hidden,
            dst: dst.to_path_buf(),
        });
    }

    fn tmpfs(&mut self, dst: &Path) {
        let origin = self.origin(dst);
        self.rows.push(PolicyRow::new(
            PolicyMode::Tmpfs,
            None,
            dst.to_path_buf(),
            origin,
        ));
        self.mounts.push(Mount {
            kind: MountKind::Tmpfs,
            dst: dst.to_path_buf(),
        });
    }

    fn row(&mut self, mode: PolicyMode, dst: &Path) {
        let origin = self.origin(dst);
        self.rows
            .push(PolicyRow::new(mode, None, dst.to_path_buf(), origin));
    }
}

/// Die Einhängungen einer Sitzung und ihre Tabelle, in der Reihenfolge, in
/// der bwrap sie schriebe (Podman ordnet beim Start selbst nach der Tiefe des
/// Ziels).
///
/// `present` sind die Pfade unter `/work`, die es im Projekt gibt; `None`
/// nimmt alle als vorhanden an (die Vorschau). Eine Maske oder ein tmpfs auf
/// einem fehlenden Pfad legte `crun` als Mountpunkt im Projekt an; sie
/// entfällt deshalb und steht in `LaunchPlan::unprotected`. `is_dir` sagt für
/// eine Quelle, ob sie ein Verzeichnis ist.
pub(crate) fn mounts(
    profile: &SandboxProfile,
    session: &SessionContext,
    sources: &Sources,
    present: Option<&BTreeSet<PathBuf>>,
    is_dir: &dyn Fn(&Path) -> bool,
    disposable: bool,
) -> (Vec<Mount>, Vec<PolicyRow>) {
    let mut out = Collector {
        mounts: Vec::new(),
        rows: Vec::new(),
        profile,
        session,
    };
    let renders = |path: &Path| present.is_none_or(|present| present.contains(path));

    for path in &profile.mounts.ro {
        out.bind(path, path, false, is_dir(path));
    }
    for link in &profile.mounts.symlinks {
        let origin = out.origin(&link.link);
        out.rows.push(PolicyRow {
            link: Some(PathBuf::from(&link.target)),
            ..PolicyRow::new(PolicyMode::Symlink, None, link.link.clone(), origin)
        });
    }
    if let Some(proc) = profile.mounts.proc.as_deref() {
        out.row(PolicyMode::Proc, proc);
    }
    if let Some(dev) = profile.mounts.dev.as_deref() {
        out.row(PolicyMode::Dev, dev);
    }
    // Podman hängt `/sys` des Rechners nur lesbar ein; unter bwrap gibt es
    // es gar nicht. Ein leeres tmpfs darüber hält die Sandbox so blind wie
    // unter bwrap (`esc-2/no_host_sys`, gemessen 2026-11-12).
    out.hide(Path::new("/sys"));
    // Podman hängt darunter `/sys/fs/cgroup` eigens ein (Eltern vor Kindern
    // sortiert); ein tmpfs über `/sys` allein verdeckt es nicht. Die Grenzen
    // der cgroup gelten trotzdem, sie hängen nicht an der Einhängung.
    out.hide(Path::new("/sys/fs/cgroup"));
    let work_dst = profile.mounts.work.dst.as_path();
    let (under_work, elsewhere): (Vec<&PathBuf>, Vec<&PathBuf>) = profile
        .mounts
        .tmpfs
        .iter()
        .partition(|path| path.starts_with(work_dst) && path.as_path() != work_dst);
    for path in elsewhere {
        out.tmpfs(path);
    }
    for (source, dst) in sources.extra_ro.iter().zip(&profile.mounts.extra_ro) {
        out.bind(source, dst, false, is_dir(source));
    }
    for (source, dst) in sources.extra_rw.iter().zip(&profile.mounts.extra_rw) {
        // Wie unter bwrap: In einer Wegwerf-Sandbox nur lesbar (HUM-350).
        out.bind(source, dst, !disposable, is_dir(source));
    }
    for (source, dst) in &sources.session_ro {
        out.bind(source, dst, false, is_dir(source));
    }
    let writable = profile.effective_work_mode(session.work_mode) == WorkMode::Rw;
    out.bind(&sources.work, work_dst, writable, true);
    for path in under_work {
        if renders(path) {
            out.tmpfs(path);
        }
    }
    let network = &profile.network;
    out.bind(
        &session.proxy_socket_src,
        &network.proxy_socket_dst,
        false,
        false,
    );
    out.bind(&session.ca_cert_src, &network.ca_cert_dst, false, false);
    out.bind(
        &session.ca_bundle_src,
        Path::new(CA_BUNDLE_DST),
        false,
        false,
    );
    out.bind(&session.shim_src, &network.shim_dst, false, false);
    if let (Some(program), Some(source)) = (&session.agent_program, &sources.agent_program) {
        out.bind(source, &program.dst, false, false);
    }
    out.file(&sources.files.passwd, Path::new(PASSWD_DST));
    out.file(&sources.files.group, Path::new(GROUP_DST));
    out.file(&sources.files.hosts, Path::new(HOSTS_DST));
    for path in profile.masks_to_render(present) {
        out.file(&sources.files.mask, &path);
    }
    for (source, file) in sources.files.agent.iter().zip(&session.files) {
        out.file(source, &file.dst);
    }
    (out.mounts, out.rows)
}

/// Lehnt eine Einhängung ab, deren Pfad sich in `--mount` nicht eindeutig
/// schreiben lässt: mit Komma, Anführungszeichen, Zeilenumbruch oder ohne
/// UTF-8. Ein Komma darin begänne ein neues Feld, etwa
/// `bind-propagation=shared`.
///
/// # Errors
///
/// `SANDBOX_006` mit dem ersten solchen Pfad.
pub(crate) fn check_expressible(paths: &[&Path]) -> Result<(), Diagnostic> {
    for path in paths {
        let text = path.to_str();
        let refused = text.is_none_or(|text| text.contains([',', '"', '\n', '\r']));
        if refused {
            return Err(Diagnostic::builder(SANDBOX_006, Severity::Blocking)
                .why(format!(
                    "the path {} holds a comma, a quote, a line break or bytes that are not \
                     UTF-8; the container engine reads mount options separated by commas, so the \
                     path cannot be mounted as it is",
                    path.display()
                ))
                .build());
        }
    }
    Ok(())
}

/// Was die leere Wurzel enthalten muss, bevor der Container startet.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Entry {
    /// Ein Verzeichnis.
    Dir(PathBuf),
    /// Eine leere Datei.
    File(PathBuf),
    /// Ein Verweis mit seinem Ziel.
    Symlink(PathBuf, PathBuf),
}

/// Die Mountpunkte, die nicht unter einer anderen Einhängung liegen, mit
/// ihren Elternverzeichnissen, die Verweise des Profils und die Ziele, die
/// Podman selbst einhängt, relativ zur Wurzel.
///
/// Legte `crun` einen fehlenden Mountpunkt selbst an, gehörte er der ersten
/// untergeordneten ID (HUM-292 P10), und das Verzeichnis der Sitzung ließe
/// sich nicht mehr löschen. Was unter einer anderen Einhängung liegt, legt
/// dort niemand an: unter einem tmpfs entsteht es im Speicher, unter einem
/// Bind muss es auf dem Host schon da sein, wie unter bwrap.
pub(crate) fn skeleton(profile: &SandboxProfile, mounts: &[Mount]) -> BTreeSet<Entry> {
    let mut entries = BTreeSet::new();
    let dsts: Vec<&Path> = mounts.iter().map(|mount| mount.dst.as_path()).collect();
    let covered = |dst: &Path| {
        dsts.iter()
            .any(|other| *other != dst && dst.starts_with(other))
    };
    for mount in mounts {
        if covered(&mount.dst) {
            continue;
        }
        let Ok(rel) = mount.dst.strip_prefix("/") else {
            continue;
        };
        add_parents(&mut entries, rel);
        let dir = match &mount.kind {
            MountKind::Tmpfs | MountKind::Hidden => true,
            MountKind::Bind { dir, .. } => *dir,
        };
        entries.insert(if dir {
            Entry::Dir(rel.to_path_buf())
        } else {
            Entry::File(rel.to_path_buf())
        });
    }
    for engine_dir in ["proc", "sys", "dev", "run"] {
        entries.insert(Entry::Dir(PathBuf::from(engine_dir)));
    }
    for engine_file in ["run/.containerenv", "etc/hostname"] {
        let rel = PathBuf::from(engine_file);
        add_parents(&mut entries, &rel);
        entries.insert(Entry::File(rel));
    }
    for link in &profile.mounts.symlinks {
        if let Ok(rel) = link.link.strip_prefix("/") {
            add_parents(&mut entries, rel);
            entries.insert(Entry::Symlink(
                rel.to_path_buf(),
                PathBuf::from(&link.target),
            ));
        }
    }
    entries
}

/// Die Elternverzeichnisse von `rel` als Einträge des Gerüsts.
fn add_parents(entries: &mut BTreeSet<Entry>, rel: &Path) {
    for parent in rel.ancestors().skip(1) {
        if !parent.as_os_str().is_empty() {
            entries.insert(Entry::Dir(parent.to_path_buf()));
        }
    }
}

/// Was die Zeile außer Profil, Sitzung und Einhängungen braucht.
#[derive(Debug, Clone)]
pub(crate) struct LineSpec<'a> {
    /// Das Programm, `argv[0]`.
    pub(crate) program: &'a Path,
    /// `--runtime`.
    pub(crate) runtime: &'a str,
    /// Das leere Verzeichnis für `--hooks-dir`.
    pub(crate) hooks_dir: &'a Path,
    /// Die leere Datei für `--default-mounts-file`: Nur dieses globale Flag
    /// ersetzt die Liste der `mounts.conf`; der Eintrag in der
    /// `containers.conf` hält die Datei des Nutzers nicht fern (gemessen
    /// 2026-11-12 unter Podman 4.9.3 und 5.8.6, HUM-454).
    pub(crate) mounts_file: &'a Path,
    /// Die eigene Kopie des seccomp-Profils von Podman.
    pub(crate) seccomp: &'a Path,
    /// Die Datei für `--conmon-pidfile`.
    pub(crate) conmon_pidfile: &'a Path,
    /// Die leere Wurzel (`--rootfs`).
    pub(crate) rootfs: &'a Path,
    /// `--name`.
    pub(crate) name: &'a str,
    /// `--label`, `schlüssel=wert`: die Instanz des Daemons.
    pub(crate) label: &'a str,
    /// `--label` mit der Sitzung; die Bestandsaufnahme beim Start des
    /// Daemons nimmt nur Container, die beide tragen.
    pub(crate) session_label: &'a str,
    /// Arbeitsspeicher in MiB.
    pub(crate) memory_mib: u64,
    /// CPUs.
    pub(crate) cpus: u32,
    /// Prozesse.
    pub(crate) pids: u64,
    /// Die Lage der Deskriptoren.
    pub(crate) fds: Fds,
    /// Wohin Ein- und Ausgabe gehen.
    pub(crate) stdio: StdioMode,
}

/// Die ganze Zeile, `argv[0]` ist das Programm.
pub(crate) fn line(
    spec: &LineSpec<'_>,
    profile: &SandboxProfile,
    session: &SessionContext,
    mounts: &[Mount],
) -> Vec<OsString> {
    let mut argv: Vec<OsString> = vec![spec.program.as_os_str().to_owned()];
    let mut push = |arg: String| argv.push(OsString::from(arg));
    // Globale Flags vor `run`.
    push(format!("--runtime={}", spec.runtime));
    push(format!("--hooks-dir={}", spec.hooks_dir.display()));
    push(format!(
        "--default-mounts-file={}",
        spec.mounts_file.display()
    ));
    push("run".to_owned());
    for flag in FIXED_RUN_FLAGS {
        push((*flag).to_owned());
    }
    push(format!("--security-opt=seccomp={}", spec.seccomp.display()));
    push(format!("--name={}", spec.name));
    push(format!("--label={}", spec.label));
    push(format!("--label={}", spec.session_label));
    push(format!("--hostname={}", profile.sandbox.hostname));
    push(format!("--memory={}m", spec.memory_mib));
    push(format!("--cpus={}", spec.cpus));
    push(format!("--pids-limit={}", spec.pids));
    push(format!(
        "--conmon-pidfile={}",
        spec.conmon_pidfile.display()
    ));
    push(format!("--preserve-fds={}", spec.fds.count()));
    push(format!("--workdir={}", profile.mounts.work.dst.display()));
    if spec.stdio == StdioMode::Inherit {
        // Die Eingabe des Aufrufers reicht die Engine sonst nicht weiter;
        // ein Terminal bekommt der Container damit nicht (kein `-t`).
        push("--interactive".to_owned());
    }
    for mount in mounts {
        push(format!("--mount={}", mount.render()));
    }
    // `--rootfs` ist ein Schalter: der Pfad steht danach an der Stelle des
    // Abbilds, nicht als `--rootfs=<pfad>` (gemessen 2026-11-12, Podman 5.8.6).
    push("--rootfs".to_owned());
    argv.push(spec.rootfs.as_os_str().to_owned());
    argv.push(profile.network.shim_dst.as_os_str().to_owned());
    let mut push = |arg: String| argv.push(OsString::from(arg));
    push("--proxy-port".to_owned());
    push(profile.network.proxy_port.to_string());
    push(crate::bridge_env::SHIM_LIFELINE_FD.to_owned());
    push(spec.fds.lifeline.to_string());
    push(crate::bridge_env::SHIM_ENV_FD.to_owned());
    push(spec.fds.env.to_string());
    push(crate::bridge_env::SHIM_PINS_FD.to_owned());
    push(spec.fds.pins.to_string());
    if let Some(stdio) = spec.fds.stdio {
        push(crate::bridge_env::SHIM_STDIO_FD.to_owned());
        push(stdio.to_string());
        push(crate::bridge_env::SHIM_CONTROLLING_TERMINAL.to_owned());
    }
    if let Some(control) = spec.fds.control {
        push(crate::bridge_env::SHIM_CONTROL_FD.to_owned());
        push(control.to_string());
    }
    push("--".to_owned());
    argv.extend(session.command.iter().cloned());
    argv
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::agent::{AgentProgram, SandboxFile};

    pub(crate) fn profile() -> SandboxProfile {
        SandboxProfile::parse(
            include_str!("../../../../../profiles/sandbox/default.toml"),
            Path::new("default.toml"),
        )
        .expect("the bundled profile")
    }

    pub(crate) fn session() -> SessionContext {
        SessionContext {
            session: humanitl_core::ids::SessionId::nil(),
            work_src: "/home/u/proj".into(),
            work_mode: WorkMode::Rw,
            proxy_socket_src: "/run/user/1000/humanitl/proxy/proxy.sock".into(),
            ca_cert_src: "/home/u/.local/share/humanitl/ca/ca.crt".into(),
            ca_bundle_src: "/home/u/.local/share/humanitl/ca/ca-bundle.crt".into(),
            shim_src: "/usr/lib/humanitl/humanitl-shim".into(),
            session_env: Vec::new(),
            command: vec!["true".into()],
            files: Vec::new(),
            agent_program: None,
        }
    }

    fn sources(profile: &SandboxProfile, session: &SessionContext) -> Sources {
        Sources {
            work: session.work_src.clone(),
            extra_ro: profile.mounts.extra_ro.clone(),
            extra_rw: profile.mounts.extra_rw.clone(),
            session_ro: Vec::new(),
            agent_program: session.agent_program.as_ref().map(|p| p.host.clone()),
            files: SessionFiles::under(Path::new("/run/s/files"), session.files.len()),
        }
    }

    fn spec(stdio: StdioMode) -> LineSpec<'static> {
        LineSpec {
            program: Path::new("/usr/bin/podman"),
            runtime: "crun",
            hooks_dir: Path::new("/run/e/hooks.d"),
            mounts_file: Path::new("/run/e/mounts.conf"),
            seccomp: Path::new("/run/e/seccomp.json"),
            conmon_pidfile: Path::new("/run/s/conmon.pid"),
            rootfs: Path::new("/run/s/rootfs"),
            name: "humanitl-0",
            label: "humanitl.daemon=abc",
            session_label: "humanitl.session=0",
            memory_mib: 2048,
            cpus: 2,
            pids: 4096,
            fds: Fds::layout(stdio.pty_size().is_some()),
            stdio,
        }
    }

    fn strings(argv: &[OsString]) -> Vec<String> {
        argv.iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    }

    /// Die Zeile ohne den Teil nach dem Shim: dort stehen die Argumente des
    /// Agenten, und die gehören nicht Podman.
    fn engine_part(argv: &[String]) -> &[String] {
        let rootfs = argv
            .iter()
            .position(|arg| arg == "--rootfs")
            .expect("--rootfs");
        &argv[..=rootfs + 1]
    }

    fn all_lines() -> Vec<Vec<String>> {
        let mut profile = profile();
        profile.mounts.extra_ro = vec!["/opt/tools".into()];
        profile.mounts.extra_rw = vec!["/home/u/cache".into()];
        let mut session = session();
        session.files = vec![SandboxFile::read_only("/home/agent/.config/a.json", "{}")];
        session.agent_program = Some(AgentProgram::new(
            "/home/u/.local/bin/opencode".into(),
            std::ffi::OsStr::new("opencode"),
        ));
        // Der Befehl des Agenten darf aussehen wie ein verbotenes Flag.
        session.command = vec!["sh".into(), "-e".into(), "-t".into()];
        let sources = sources(&profile, &session);
        let mut lines = Vec::new();
        for stdio in [
            StdioMode::Inherit,
            StdioMode::Capture,
            StdioMode::Pty { cols: 80, rows: 24 },
        ] {
            for disposable in [false, true] {
                let (mounts, _) = mounts(&profile, &session, &sources, None, &|_| true, disposable);
                lines.push(strings(&line(&spec(stdio), &profile, &session, &mounts)));
            }
        }
        lines
    }

    /// Jedes feste Flag genau einmal, und nichts aus der Verbotsliste, in
    /// jeder Form, die eine Zeile annehmen kann (HUM-433).
    #[test]
    fn every_fixed_flag_appears_exactly_once_and_nothing_forbidden() {
        for argv in all_lines() {
            let engine = engine_part(&argv);
            for flag in FIXED_RUN_FLAGS {
                let count = engine.iter().filter(|arg| arg.as_str() == *flag).count();
                assert_eq!(count, 1, "{flag} appears {count} times in {engine:?}");
            }
            for arg in engine {
                for forbidden in FORBIDDEN {
                    assert!(
                        !(arg == forbidden || arg.starts_with(forbidden)),
                        "{arg} is forbidden ({forbidden})"
                    );
                }
                if arg.starts_with("--network") {
                    assert_eq!(arg, "--network=none");
                }
                assert!(!arg.ends_with(":z") && !arg.ends_with(":Z"), "{arg}");
                assert!(!arg.contains("relabel"), "{arg}");
            }
            for once in [
                "--name=",
                "--label=humanitl.daemon=",
                "--label=humanitl.session=",
                "--memory=",
                "--cpus=",
                "--pids-limit=",
                "--conmon-pidfile=",
                "--preserve-fds=",
                "--security-opt=seccomp=",
                "--hostname=",
            ] {
                let count = engine.iter().filter(|arg| arg.starts_with(once)).count();
                assert_eq!(count, 1, "{once} appears {count} times");
            }
        }
    }

    /// Die Flags aus der Spezifikation von HUM-433, hier wörtlich und nicht
    /// aus [`FIXED_RUN_FLAGS`] gelesen: Fällt eines aus der Liste, wird
    /// dieser Test rot, nicht nur die Liste kürzer.
    #[test]
    fn the_line_carries_every_flag_the_guarantees_rest_on() {
        const REQUIRED: &[&str] = &[
            "--rm",
            "--network=none",
            "--ipc=private",
            "--uts=private",
            "--pid=private",
            "--cgroupns=private",
            "--userns=keep-id",
            "--cap-drop=all",
            "--security-opt=no-new-privileges",
            "--read-only",
            "--read-only-tmpfs=false",
            "--log-driver=none",
            "--pull=never",
            "--no-hosts",
            "--env-host=false",
            "--http-proxy=false",
        ];
        for argv in all_lines() {
            let engine = engine_part(&argv);
            for flag in REQUIRED {
                assert!(engine.iter().any(|arg| arg == flag), "{flag} missing");
            }
            assert!(engine.iter().any(|arg| arg.starts_with("--hooks-dir=")));
            assert!(
                argv.iter()
                    .take_while(|arg| *arg != "run")
                    .any(|arg| arg == "--default-mounts-file=/run/e/mounts.conf"),
                "the global flag --default-mounts-file must precede run (HUM-454)"
            );
            assert!(engine.iter().any(|arg| arg == "--rootfs"));
        }
    }

    /// Die Tabelle trifft die Verbotsliste: ohne `--network=none` oder mit
    /// einem `-e` färbt sie rot. Das ist der Beweis, dass der Test oben
    /// etwas prüft.
    #[test]
    fn the_forbidden_list_catches_what_it_names() {
        let caught = |arg: &str| {
            FORBIDDEN
                .iter()
                .any(|forbidden| arg == *forbidden || arg.starts_with(forbidden))
        };
        for arg in [
            "-e",
            "--env=A=b",
            "-t",
            "--privileged",
            "--cap-add=SYS_ADMIN",
            "--device=/dev/kvm",
            "--volume=/x:/y:z",
            "--security-opt=label=disable",
            "--net=host",
        ] {
            assert!(caught(arg), "{arg}");
        }
        for arg in ["--network=none", "--read-only", "--env-host=false"] {
            assert!(!caught(arg), "{arg}");
        }
    }

    /// Ein Profil mit `mounts.extra_rw` hat genau einen beschreibbaren Bind
    /// außer `/work`, und in einer Wegwerf-Sandbox keinen.
    #[test]
    fn extra_rw_gives_exactly_one_writable_bind_besides_work() {
        let mut profile = profile();
        profile.mounts.extra_rw = vec!["/home/u/cache".into()];
        let session = session();
        let sources = sources(&profile, &session);
        let writable = |disposable| {
            let (mounts, _) = mounts(&profile, &session, &sources, None, &|_| true, disposable);
            mounts
                .into_iter()
                .filter(|mount| {
                    matches!(mount.kind, MountKind::Bind { writable: true, .. })
                        && mount.dst != Path::new("/work")
                })
                .map(|mount| mount.dst)
                .collect::<Vec<_>>()
        };
        assert_eq!(writable(false), [PathBuf::from("/home/u/cache")]);
        assert!(writable(true).is_empty());
        profile.mounts.extra_rw.clear();
        let (mounts, _) = mounts(&profile, &session, &sources, None, &|_| true, false);
        let writable: Vec<_> = mounts
            .iter()
            .filter(|mount| matches!(mount.kind, MountKind::Bind { writable: true, .. }))
            .collect();
        assert_eq!(writable.len(), 1, "only /work: {writable:?}");
    }

    /// Podman hängt `/sys` des Rechners und darunter `/sys/fs/cgroup` ein,
    /// bwrap nichts davon: Jede Zeile verdeckt beide mit einem leeren, nur
    /// lesbaren tmpfs, das Kind nach dem Eltern, und die Vorschau nennt
    /// beide maskiert (HUM-453).
    #[test]
    fn every_line_hides_the_hosts_sys() {
        for argv in all_lines() {
            let engine = engine_part(&argv);
            let at = |dst: &str| {
                let want =
                    format!("--mount=type=tmpfs,destination={dst},ro,tmpfs-mode=0555,notmpcopyup");
                engine.iter().position(|arg| *arg == want)
            };
            let sys = at("/sys").unwrap_or_else(|| panic!("/sys: {argv:?}"));
            let cgroup = at("/sys/fs/cgroup").unwrap_or_else(|| panic!("cgroup: {argv:?}"));
            assert!(sys < cgroup, "the parent first: {argv:?}");
        }
        let profile = profile();
        let session = session();
        let sources = sources(&profile, &session);
        let (_, rows) = mounts(&profile, &session, &sources, None, &|_| true, false);
        for dst in ["/sys", "/sys/fs/cgroup"] {
            assert!(
                rows.iter()
                    .any(|row| row.mode == PolicyMode::Masked && row.target == Path::new(dst)),
                "{dst} masked in the preview: {rows:?}"
            );
        }
    }

    /// Masken und tmpfs unter `/work` nur auf vorhandenen Pfaden; was fehlt,
    /// bekommt keine Einhängung (sonst legte `crun` den Mountpunkt im
    /// Projekt an).
    #[test]
    fn masks_and_tmpfs_under_work_only_on_present_paths() {
        let profile = profile();
        let session = session();
        let sources = sources(&profile, &session);
        let present: BTreeSet<PathBuf> = [
            PathBuf::from("/work/.git/hooks"),
            PathBuf::from("/work/.env"),
        ]
        .into_iter()
        .collect();
        let (mounts, rows) = mounts(
            &profile,
            &session,
            &sources,
            Some(&present),
            &|_| true,
            false,
        );
        let under_work: Vec<&Path> = mounts
            .iter()
            .map(|mount| mount.dst.as_path())
            .filter(|dst| dst.starts_with("/work") && *dst != Path::new("/work"))
            .collect();
        assert_eq!(
            under_work,
            [Path::new("/work/.git/hooks"), Path::new("/work/.env")]
        );
        // Jede Einhängung hat ihre Zeile; dazu kommen Verweise, `/proc`
        // und `/dev`, die Podman selbst stellt.
        let mounted: Vec<&PolicyRow> = rows
            .iter()
            .filter(|row| {
                !matches!(
                    row.mode,
                    PolicyMode::Symlink | PolicyMode::Proc | PolicyMode::Dev
                )
            })
            .collect();
        assert_eq!(mounts.len(), mounted.len());
        for (mount, row) in mounts.iter().zip(mounted) {
            assert_eq!(mount.dst, row.target);
        }
    }

    /// Die Zeile nennt die Deskriptoren, die der Start fortlaufend ab 3
    /// vergibt: am eigenen Pseudoterminal sechs, sonst vier.
    #[test]
    fn the_descriptors_are_named_in_the_order_they_are_handed_over() {
        for (stdio, count) in [
            (StdioMode::Capture, 4),
            (StdioMode::Pty { cols: 80, rows: 24 }, 6),
        ] {
            let profile = profile();
            let session = session();
            let (mounts, _) = mounts(
                &profile,
                &session,
                &sources(&profile, &session),
                None,
                &|_| true,
                false,
            );
            let argv = strings(&line(&spec(stdio), &profile, &session, &mounts));
            assert!(
                argv.contains(&format!("--preserve-fds={count}")),
                "{argv:?}"
            );
            let shim = argv
                .iter()
                .position(|arg| arg == "/run/humanitl/humanitl-shim")
                .expect("the shim");
            let tail = argv[shim..].join(" ");
            assert!(
                tail.contains("--lifeline-fd 4 --env-fd 5 --pins-fd 6"),
                "{tail}"
            );
            assert_eq!(
                tail.contains("--stdio-fd 8 --controlling-terminal --control-fd 7"),
                count == 6,
                "{tail}"
            );
            assert!(tail.ends_with("-- true"), "{tail}");
        }
    }

    #[test]
    fn a_path_with_a_comma_cannot_be_mounted() {
        let err = check_expressible(&[Path::new("/home/u/a,relabel=shared")]).expect_err("comma");
        assert_eq!(err.code.as_str(), "SANDBOX_006", "{}", err.why);
        check_expressible(&[Path::new("/home/u/a b=c")]).expect("plain");
    }

    /// Die Wurzel bekommt jeden Mountpunkt, der nicht unter einer anderen
    /// Einhängung liegt, die Verweise und die Ziele von Podman.
    #[test]
    fn the_skeleton_holds_every_top_mount_point_and_no_nested_one() {
        let profile = profile();
        let session = session();
        let is_dir =
            |p: &Path| p != Path::new("/etc/ld.so.cache") && p != Path::new("/etc/localtime");
        let (mounts, _) = mounts(
            &profile,
            &session,
            &sources(&profile, &session),
            None,
            &is_dir,
            false,
        );
        let entries = skeleton(&profile, &mounts);
        for expected in [
            Entry::Dir("usr".into()),
            Entry::Dir("etc/ssl".into()),
            Entry::File("etc/ld.so.cache".into()),
            Entry::Dir("work".into()),
            Entry::Dir("home/agent".into()),
            Entry::File("run/humanitl/proxy.sock".into()),
            Entry::File("run/humanitl/humanitl-shim".into()),
            Entry::File("etc/passwd".into()),
            Entry::File("etc/hostname".into()),
            Entry::File("run/.containerenv".into()),
            Entry::Dir("proc".into()),
            Entry::Symlink("lib".into(), "usr/lib".into()),
        ] {
            assert!(entries.contains(&expected), "{expected:?} missing");
        }
        // Unter `/work` und unter `/etc/ssl` legt niemand etwas an.
        let nested = |p: &Path| {
            ["work", "etc/ssl"]
                .iter()
                .any(|top| p.starts_with(top) && p != Path::new(top))
        };
        let found: Vec<&Entry> = entries
            .iter()
            .filter(|entry| matches!(entry, Entry::Dir(p) | Entry::File(p) if nested(p)))
            .collect();
        assert!(found.is_empty(), "{found:?}");
    }
}
