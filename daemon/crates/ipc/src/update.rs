//! Updates von Humanitl selbst, die Seite des Daemons ohne Netz (HUM-382).
//!
//! Das Netz und die Signatur liegen im Proxy-Crate (`self_update`): Dort steht
//! der HTTPS-Stapel, und dort wird das Manifest gegen den einkompilierten
//! Schlüssel geprüft. Hier steht, was danach kommt und was davor entschieden
//! wird:
//!
//! - **Wie diese Installation ein Update bekommt** ([`detect`]): unter macOS
//!   das Bündel `Humanitl.app`, unter Linux der entpackte Archiv-Ordner, ein
//!   `AppImage`, ein Paket aus apt oder dnf, oder kein Weg aus der App (AUR,
//!   Paket ohne Repository, ein Ordner ohne Schreibrecht). Gelesen wird nur
//!   der eigene Pfad und, für Pakete, die Ablage des Paketmanagers.
//! - **Wann gefragt wird** ([`Updater`]): höchstens einmal am Tag, und ohne
//!   `force` nur, wenn `update.check` an ist.
//! - **Wie getauscht wird** ([`install`]): entpacken, prüfen (Signatur des
//!   Bündels unter macOS, Aufbau und Version des Archivs unter Linux), neben
//!   das Ziel kopieren, das Ziel zu `.previous` umbenennen, das neue an seinen
//!   Platz, und erst danach `.previous` löschen. Scheitert der zweite Schritt,
//!   kommt `.previous` zurück.
//!
//! Ein Paket aus apt oder dnf installiert der Daemon nie selbst: `pkexec`
//! braucht den Polkit-Agenten der Sitzung, und den hat ein Dienst nicht. Der
//! Client startet den festen Aufruf aus [`apt_command`] oder [`dnf_command`];
//! der Paketmanager prüft seine eigenen Signaturen.

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use humanitl_core::diagnostics::codes::{UPDATE_002, UPDATE_003, UPDATE_004, UPDATE_005};
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::convert::diagnostic_to_proto;
use crate::v1;

/// Höchstens so oft fragt der Daemon von sich aus.
pub const CHECK_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

/// Die Seite mit allen Releases, für den Weg von Hand.
const RELEASES_PAGE: &str = "https://github.com/nurkert/Humanitl/releases";

/// Der Paketname in apt, dnf und im Archiv.
const PACKAGE: &str = "humanitl";

/// Der feste Aufruf für ein Paket aus dem apt-Repository.
#[must_use]
pub fn apt_command() -> Vec<String> {
    [
        "pkexec",
        "apt-get",
        "install",
        "--only-upgrade",
        "-y",
        PACKAGE,
    ]
    .map(str::to_owned)
    .to_vec()
}

/// Der feste Aufruf für ein Paket aus dem dnf-Repository.
#[must_use]
pub fn dnf_command() -> Vec<String> {
    ["pkexec", "dnf", "upgrade", "-y", PACKAGE]
        .map(str::to_owned)
        .to_vec()
}

/// Wie diese Installation ein Update bekommt.
#[derive(Debug, Clone, PartialEq)]
pub enum Install {
    /// macOS: dieses Bündel wird ersetzt.
    AppBundle(PathBuf),
    /// Linux: dieser entpackte Archiv-Ordner wird ersetzt.
    Archive(PathBuf),
    /// Linux: dieses `AppImage` wird ersetzt.
    AppImage(PathBuf),
    /// Paket aus dem apt-Repository.
    Apt,
    /// Paket aus dem dnf-Repository.
    Dnf,
    /// Kein Weg aus der App; der Befund nennt den Befehl zum Kopieren.
    Manual(Box<Diagnostic>),
}

impl Install {
    /// Das Manifest des letzten Releases, das zu diesem Weg gehört. Ein
    /// Paket hat keines; seine Version sagt das Manifest des Archivs, das
    /// derselbe Bau ist.
    #[must_use]
    pub fn manifest(&self) -> &'static str {
        match self {
            Self::AppBundle(_) => "update-macos.json",
            Self::AppImage(_) => "update-linux-x86_64-appimage.json",
            Self::Manual(_) if cfg!(target_os = "macos") => "update-macos.json",
            Self::Archive(_) | Self::Apt | Self::Dnf | Self::Manual(_) => {
                "update-linux-x86_64.json"
            }
        }
    }

    /// Der Weg im Vertrag.
    #[must_use]
    pub fn method(&self) -> v1::UpdateMethod {
        match self {
            Self::AppBundle(_) => v1::UpdateMethod::AppBundle,
            Self::Archive(_) => v1::UpdateMethod::Archive,
            Self::AppImage(_) => v1::UpdateMethod::Appimage,
            Self::Apt => v1::UpdateMethod::Apt,
            Self::Dnf => v1::UpdateMethod::Dnf,
            Self::Manual(_) => v1::UpdateMethod::Manual,
        }
    }

    /// Der Aufruf, den der Client startet; leer außer bei apt und dnf.
    #[must_use]
    pub fn upgrade_command(&self) -> Vec<String> {
        match self {
            Self::Apt => apt_command(),
            Self::Dnf => dnf_command(),
            _ => Vec::new(),
        }
    }

    /// Wahr, wenn `Apply` diesen Weg im Daemon geht.
    #[must_use]
    pub const fn applies_in_daemon(&self) -> bool {
        matches!(
            self,
            Self::AppBundle(_) | Self::Archive(_) | Self::AppImage(_)
        )
    }
}

/// Was [`detect`] vom Rechner liest. Im Daemon `/`; Tests bauen einen Baum
/// unter einem Wegwerf-Verzeichnis.
#[derive(Debug, Clone)]
pub struct Host {
    /// Wurzel, unter der `/var/lib/dpkg`, `/etc/apt` und `/usr/bin/pkexec`
    /// gesucht werden.
    pub root: PathBuf,
    /// Wahr unter macOS.
    pub macos: bool,
}

impl Host {
    /// Der Rechner, auf dem der Daemon läuft.
    #[must_use]
    pub fn this() -> Self {
        Self {
            root: PathBuf::from("/"),
            macos: cfg!(target_os = "macos"),
        }
    }

    fn at(&self, absolute: &str) -> PathBuf {
        self.root.join(absolute.trim_start_matches('/'))
    }

    /// Wahr, wenn eine Datei unter `dir` den Text `needle` enthält.
    fn any_file_mentions(&self, dir: &str, needle: &str) -> bool {
        let Ok(entries) = std::fs::read_dir(self.at(dir)) else {
            return false;
        };
        entries.flatten().any(|entry| {
            std::fs::read_to_string(entry.path()).is_ok_and(|text| text.contains(needle))
        })
    }
}

/// `UPDATE_004` mit `fix`.
fn not_here(why: String, fix: FixAction) -> Diagnostic {
    Diagnostic::builder(UPDATE_004, Severity::Info)
        .why(why)
        .fix(fix)
        .build()
}

fn releases_page() -> FixAction {
    FixAction::OpenUrl(RELEASES_PAGE.to_owned())
}

/// Wie die Installation, aus der `exe` läuft, ein Update bekommt.
///
/// `exe` ist der Pfad des Daemons (`current_exe`), `appimage` der Pfad, den
/// die App aus `$APPIMAGE` mitschickt. Nichts hier schreibt außer der Probe,
/// ob ein Ordner beschreibbar ist (eine leere Datei, sofort gelöscht).
#[must_use]
pub fn detect(exe: &Path, appimage: Option<&Path>, host: &Host) -> Install {
    if host.macos {
        return match bundle_of(exe) {
            Ok(bundle) => match writable_parent(&bundle) {
                Ok(()) => Install::AppBundle(bundle),
                Err(why) => Install::Manual(Box::new(not_here(why, releases_page()))),
            },
            Err(diagnostic) => Install::Manual(Box::new(diagnostic)),
        };
    }
    if let Some(image) = appimage {
        return match appimage_target(image) {
            Ok(file) => Install::AppImage(file),
            Err(diagnostic) => Install::Manual(Box::new(diagnostic)),
        };
    }
    if exe.starts_with("/usr") {
        return package(host);
    }
    if exe.to_string_lossy().contains("/.local/lib/humanitl/") {
        return Install::Manual(Box::new(not_here(
            "this daemon is the copy an AppImage installed; the AppImage itself is updated \
             from the app it starts"
                .to_owned(),
            releases_page(),
        )));
    }
    match archive_root(exe) {
        Some(root) => match writable_parent(&root) {
            Ok(()) => Install::Archive(root),
            Err(why) => Install::Manual(Box::new(not_here(why, releases_page()))),
        },
        None => Install::Manual(Box::new(not_here(
            format!(
                "{} is neither an installed package nor an unpacked release archive, so there \
                 is nothing this daemon may replace",
                exe.display()
            ),
            releases_page(),
        ))),
    }
}

/// Der Name des APT-Repositorys, das Humanitl ausliefert (`README.md`,
/// `docs/INSTALL.md`): Das Installationsskript legt
/// `/etc/apt/sources.list.d/nurkert.list` mit diesem Namen an.
const APT_REPOSITORY: &str = "apt.nurkert.de";

/// Ob apt Humanitl aus dem eigenen Repository kennt: eine Quelle nennt
/// [`APT_REPOSITORY`] (Einzeiler `.list` oder deb822 `.sources`, auch in
/// `/etc/apt/sources.list`), oder eine geladene Paketliste dieses
/// Repositorys führt das Paket `humanitl`. Eine auskommentierte Zeile zählt
/// nicht.
fn apt_repository_serves_humanitl(host: &Host) -> bool {
    let names_repository = |text: &str| {
        text.lines().any(|line| {
            let line = line.trim_start();
            !line.starts_with('#') && line.contains(APT_REPOSITORY)
        })
    };
    let in_sources = std::fs::read_dir(host.at("/etc/apt/sources.list.d")).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            let path = entry.path();
            matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("list" | "sources")
            ) && std::fs::read_to_string(&path).is_ok_and(|text| names_repository(&text))
        })
    }) || std::fs::read_to_string(host.at("/etc/apt/sources.list"))
        .is_ok_and(|text| names_repository(&text));
    in_sources
        || std::fs::read_dir(host.at("/var/lib/apt/lists")).is_ok_and(|entries| {
            entries.flatten().any(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.contains(APT_REPOSITORY)
                    && name.ends_with("_Packages")
                    && std::fs::read_to_string(entry.path()).is_ok_and(|text| {
                        text.lines()
                            .any(|line| line.trim_end() == "Package: humanitl")
                    })
            })
        })
}

/// Ein Paket: apt, dnf, AUR oder keins davon.
fn package(host: &Host) -> Install {
    let pkexec = host.at("/usr/bin/pkexec").is_file();
    if host.at("/var/lib/dpkg/info/humanitl.list").is_file() {
        let repo = apt_repository_serves_humanitl(host);
        return match (repo, pkexec) {
            (true, true) => Install::Apt,
            (true, false) => Install::Manual(Box::new(not_here(
                "Humanitl came from the apt repository, but pkexec is not installed, so the app \
                 cannot ask for the administrator password"
                    .to_owned(),
                FixAction::CopyCommand("sudo apt-get install --only-upgrade humanitl".to_owned()),
            ))),
            (false, _) => Install::Manual(Box::new(not_here(
                "Humanitl was installed from a .deb file without the apt repository, so apt \
                 knows no newer version; download the new .deb from the releases page"
                    .to_owned(),
                releases_page(),
            ))),
        };
    }
    let pacman = std::fs::read_dir(host.at("/var/lib/pacman/local")).is_ok_and(|entries| {
        entries.flatten().any(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("humanitl-bin-")
        })
    });
    if pacman {
        return Install::Manual(Box::new(not_here(
            "Humanitl came from the AUR package humanitl-bin; update it with your AUR helper \
             or makepkg, the app never runs one"
                .to_owned(),
            FixAction::CopyCommand("yay -S humanitl-bin".to_owned()),
        )));
    }
    let rpm = host.at("/var/lib/rpm").is_dir() || host.at("/usr/lib/sysimage/rpm").is_dir();
    if rpm {
        let repo = host.any_file_mentions("/etc/yum.repos.d", "humanitl");
        return match (repo, pkexec) {
            (true, true) => Install::Dnf,
            (true, false) => Install::Manual(Box::new(not_here(
                "Humanitl came from the dnf repository, but pkexec is not installed, so the app \
                 cannot ask for the administrator password"
                    .to_owned(),
                FixAction::CopyCommand("sudo dnf upgrade humanitl".to_owned()),
            ))),
            (false, _) => Install::Manual(Box::new(not_here(
                "Humanitl was installed from an .rpm file without the dnf repository, so dnf \
                 knows no newer version; download the new .rpm from the releases page"
                    .to_owned(),
                releases_page(),
            ))),
        };
    }
    Install::Manual(Box::new(not_here(
        "Humanitl lies under /usr, but no package manager this daemon knows claims it".to_owned(),
        releases_page(),
    )))
}

/// Der Archiv-Ordner über `<root>/bin/humanitld`, wenn er wie ein entpacktes
/// Release aussieht (`bin/`, `app/`, `share/humanitl/catalog`).
fn archive_root(exe: &Path) -> Option<PathBuf> {
    let bin = exe.parent()?;
    if bin.file_name()? != "bin" {
        return None;
    }
    let root = bin.parent()?;
    (root.join("app").is_dir() && root.join("share/humanitl/catalog").is_dir())
        .then(|| root.to_path_buf())
}

/// Prüft das `AppImage`, das die App nennt.
fn appimage_target(image: &Path) -> Result<PathBuf, Diagnostic> {
    let refuse = |why: String| not_here(why, releases_page());
    if !image.is_absolute()
        || image.starts_with("/usr")
        || image.starts_with("/opt")
        || image.starts_with("/tmp/.mount_")
    {
        return Err(refuse(format!(
            "{} is not an AppImage in a folder of this user",
            image.display()
        )));
    }
    let meta = std::fs::symlink_metadata(image)
        .map_err(|err| refuse(format!("{} cannot be read: {err}", image.display())))?;
    let named = image
        .file_name()
        .is_some_and(|name| name.to_string_lossy().ends_with(".AppImage"));
    if !meta.is_file() || !named {
        return Err(refuse(format!(
            "{} is not a regular file named *.AppImage",
            image.display()
        )));
    }
    writable_parent(image).map_err(refuse)?;
    Ok(image.to_path_buf())
}

/// Das Bündel über `<bundle>/Contents/MacOS/bin/humanitld`.
///
/// # Errors
///
/// `UPDATE_004`, wenn der Daemon nicht aus einem `.app` läuft, aus einem
/// Disk-Image unter `/Volumes` oder aus App Translocation.
pub fn bundle_of(exe: &Path) -> Result<PathBuf, Diagnostic> {
    let bundle = exe
        .ancestors()
        .find(|dir| dir.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            not_here(
                format!(
                    "{} does not run from an app bundle, so there is nothing to replace",
                    exe.display()
                ),
                releases_page(),
            )
        })?;
    let text = bundle.to_string_lossy();
    if bundle.starts_with("/Volumes") || text.contains("/AppTranslocation/") {
        return Err(not_here(
            format!(
                "{} runs from a disk image or from App Translocation; move Humanitl.app to \
                 /Applications and start it from there",
                bundle.display()
            ),
            releases_page(),
        ));
    }
    Ok(bundle)
}

/// Ob der Ordner über `target` für diesen Nutzer beschreibbar ist.
fn writable_parent(target: &Path) -> Result<(), String> {
    let parent = target
        .parent()
        .ok_or_else(|| format!("{} has no parent folder", target.display()))?;
    let probe = parent.join(format!(".humanitl-update-probe-{}", std::process::id()));
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(err) => Err(format!(
            "{} may not be written by this user ({err}), so the update cannot replace {}",
            parent.display(),
            target.display()
        )),
    }
}

/// Was der letzte Blick auf GitHub ergab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    /// Die Version des letzten Releases.
    pub version: String,
    /// Wahr, wenn sie neuer ist als dieser Daemon.
    pub newer: bool,
    /// Größe der Datei in Bytes.
    pub size: u64,
}

#[derive(Debug, Default)]
struct Cached {
    attempted_at: Option<SystemTime>,
    checked_at: Option<SystemTime>,
    offer: Option<Offer>,
}

/// Der Stand der Updates in diesem Daemon.
#[derive(Debug)]
pub struct Updater {
    current: String,
    cached: Mutex<Cached>,
    /// Hält ein laufendes `Apply`; ein zweites wird abgelehnt.
    pub applying: std::sync::Arc<tokio::sync::Mutex<()>>,
}

impl Updater {
    /// Ein Stand ohne Antwort, für diese Version.
    #[must_use]
    pub fn new(current: &str) -> Self {
        Self {
            current: current.to_owned(),
            cached: Mutex::new(Cached::default()),
            applying: std::sync::Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Cached> {
        self.cached
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Ob eine Frage ohne `force` jetzt fällig ist: noch nie gefragt, oder
    /// der letzte Versuch liegt mindestens [`CHECK_INTERVAL`] zurück. Auch
    /// ein gescheiterter Versuch zählt, damit ein schweigendes GitHub nicht
    /// stündlich gefragt wird.
    #[must_use]
    pub fn due(&self, now: SystemTime) -> bool {
        self.lock().attempted_at.is_none_or(|at| {
            now.duration_since(at)
                .map_or(true, |since| since >= CHECK_INTERVAL)
        })
    }

    /// Vermerkt einen Versuch.
    pub fn attempted(&self, now: SystemTime) {
        self.lock().attempted_at = Some(now);
    }

    /// Vermerkt eine Antwort.
    pub fn record(&self, offer: Offer, now: SystemTime) {
        let mut cached = self.lock();
        cached.checked_at = Some(now);
        cached.offer = Some(offer);
    }

    /// Der Stand im Vertrag, für diese Installation.
    #[must_use]
    pub fn status(&self, install: &Install) -> v1::UpdateStatus {
        let cached = self.lock();
        let offer = cached.offer.as_ref();
        let available = offer.is_some_and(|offer| offer.newer);
        let notice = match install {
            Install::Manual(diagnostic) if available => Some(diagnostic_to_proto(diagnostic)),
            _ => None,
        };
        v1::UpdateStatus {
            current_version: self.current.clone(),
            latest_version: offer.map(|offer| offer.version.clone()).unwrap_or_default(),
            available,
            can_apply: available && !matches!(install, Install::Manual(_)),
            checked_at: cached.checked_at.map(prost_types::Timestamp::from),
            size: if available {
                offer.map_or(0, |offer| offer.size)
            } else {
                0
            },
            notice,
            method: install.method().into(),
            upgrade_command: install.upgrade_command(),
        }
    }
}

/// Warum `Apply` diesen Weg nicht im Daemon geht; `None`, wenn er es tut.
#[must_use]
pub fn refusal(install: &Install) -> Option<Diagnostic> {
    match install {
        Install::Manual(diagnostic) => Some((**diagnostic).clone()),
        Install::Apt | Install::Dnf => Some(
            Diagnostic::builder(UPDATE_004, Severity::Error)
                .why(
                    "this package is updated by its package manager; the client runs \
                     UpdateStatus.upgrade_command in the user's session"
                        .to_owned(),
                )
                .fix(FixAction::CopyCommand(install.upgrade_command().join(" ")))
                .build(),
        ),
        Install::AppBundle(_) | Install::Archive(_) | Install::AppImage(_) => None,
    }
}

/// Eine Version `MAJOR.MINOR.PATCH`, nur Ziffern, führendes `v` erlaubt.
pub type Triple = (u64, u64, u64);

/// Liest `1.2.3` wie das Manifest; `None` für alles andere.
#[must_use]
pub fn parse_version(text: &str) -> Option<Triple> {
    let text = text.trim();
    let text = text.strip_prefix('v').unwrap_or(text);
    let mut parts = text.split('.').map(|part| {
        (!part.is_empty()
            && part.bytes().all(|b| b.is_ascii_digit())
            && (part.len() == 1 || !part.starts_with('0')))
        .then(|| part.parse::<u64>().ok())
        .flatten()
    });
    let version = (parts.next()??, parts.next()??, parts.next()??);
    parts.next().is_none().then_some(version)
}

/// Der Pfad, den ein Weg ersetzt.
fn target_of(install: &Install) -> Option<&Path> {
    match install {
        Install::AppBundle(path) | Install::Archive(path) | Install::AppImage(path) => Some(path),
        Install::Apt | Install::Dnf | Install::Manual(_) => None,
    }
}

/// Die Datei unter dem Datenverzeichnis, in die `humanitl daemon install`
/// bei jedem Start eines `AppImage`s dessen Pfad (`$APPIMAGE`) schreibt, mit
/// `0600` (HUM-382).
pub const APPIMAGE_RECORD: &str = "appimage-path";

/// Ob der Pfad eines `AppImage`s aus einer Anfrage einer ist, dem der Daemon
/// trauen darf: der Pfad, den `daemon install` aus dem `AppImage` selbst
/// vermerkt hat ([`APPIMAGE_RECORD`], nur wenn weder Gruppe noch andere sie
/// schreiben dürfen), oder ein Ziel, das schon im Register von
/// [`record_installed`] steht. Ein beliebiger Pfad aus der Anfrage ist es
/// nicht: Mit ihm würde der Daemon fremde Dateien zurückholen oder ersetzen
/// (Review zu HUM-382).
#[must_use]
pub fn appimage_trusted(request: &Path, record: &Path, ledger: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    let text = request.display().to_string();
    let recorded = std::fs::symlink_metadata(record)
        .ok()
        .filter(|meta| meta.is_file() && meta.mode() & 0o022 == 0)
        .and_then(|_| std::fs::read_to_string(record).ok())
        .is_some_and(|content| content.trim_end_matches('\n') == text);
    recorded || read_ledger(ledger).contains_key(&text)
}

/// Das Register, welche Version zuletzt an welchen Pfad installiert wurde.
fn read_ledger(ledger: &Path) -> serde_json::Map<String, serde_json::Value> {
    std::fs::read(ledger)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value.as_object().cloned())
        .unwrap_or_default()
}

/// Die Untergrenze für ein Update an diesem Ziel: die höchste von drei
/// Versionen, und angeboten wird nur, was über ihr liegt.
///
/// - die Version dieses Daemons;
/// - die Version dessen, was jetzt installiert ist (unter macOS aus dem
///   `Info.plist` des Bündels);
/// - die höchste Version, die dieser Daemon je an dieses Ziel installiert
///   hat ([`record_installed`]).
///
/// Ohne die beiden letzten wäre ein Downgrade ohne Schlüssel möglich: Ein
/// Daemon 0.0.5 installiert 0.0.7, sein Neustart wartet auf das Ende einer
/// Sitzung, und ein echtes, gültig signiertes 0.0.6 wäre aus seiner Sicht
/// noch neuer (Review zu HUM-382).
#[must_use]
pub fn installed_floor(install: &Install, current: &str, ledger: &Path) -> Option<Triple> {
    let mut floor = parse_version(current);
    let mut raise = |candidate: Option<Triple>| {
        if let Some(candidate) = candidate {
            floor = Some(floor.map_or(candidate, |known| known.max(candidate)));
        }
    };
    if let Install::AppBundle(bundle) = install {
        raise(bundle_version(bundle).as_deref().and_then(parse_version));
    }
    if let Some(target) = target_of(install) {
        let entries = read_ledger(ledger);
        raise(
            entries
                .get(&target.display().to_string())
                .and_then(serde_json::Value::as_str)
                .and_then(parse_version),
        );
    }
    floor
}

/// Ob `version` über `floor` liegt; eine unlesbare Version nie.
#[must_use]
pub fn above_floor(version: &str, floor: Option<Triple>) -> bool {
    parse_version(version).is_some_and(|version| floor.is_none_or(|floor| version > floor))
}

/// Vermerkt nach einem gelungenen Tausch, dass `version` an diesem Ziel
/// liegt. Das Register steigt nur: eine kleinere Version überschreibt nie
/// eine größere. Geschrieben über eine Nachbardatei und `rename`, mit `0600`.
///
/// # Errors
///
/// `UPDATE_005`, wenn das Register nicht geschrieben werden konnte.
pub fn record_installed(ledger: &Path, install: &Install, version: &str) -> Result<(), Diagnostic> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let failed = |why: String| {
        install_failed(format!(
            "the installed version could not be recorded in {}: {why}",
            ledger.display()
        ))
    };
    let Some(target) = target_of(install) else {
        return Ok(());
    };
    let mut entries = read_ledger(ledger);
    let key = target.display().to_string();
    let known = entries
        .get(&key)
        .and_then(serde_json::Value::as_str)
        .and_then(parse_version);
    if known.is_some_and(|known| parse_version(version).is_none_or(|new| new <= known)) {
        return Ok(());
    }
    entries.insert(key, serde_json::Value::String(version.to_owned()));
    if let Some(parent) = ledger.parent() {
        std::fs::create_dir_all(parent).map_err(|err| failed(err.to_string()))?;
    }
    let next = sibling(ledger, ".next");
    let text = serde_json::to_vec_pretty(&serde_json::Value::Object(entries))
        .map_err(|err| failed(err.to_string()))?;
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&next)
        .map_err(|err| failed(format!("{}: {err}", next.display())))?;
    file.write_all(&text)
        .and_then(|()| file.sync_all())
        .map_err(|err| failed(format!("{}: {err}", next.display())))?;
    std::fs::rename(&next, ledger).map_err(|err| failed(err.to_string()))
}

/// Prüft das neu gelesene Manifest gegen das, was der Client angeboten hat,
/// und gegen die Untergrenze aus [`installed_floor`].
///
/// # Errors
///
/// `UPDATE_002`, wenn das Manifest jetzt eine andere Version nennt als die
/// angebotene, oder wenn sie nicht über der Untergrenze liegt.
pub fn offer_matches(
    offered: &str,
    manifest: &str,
    floor: Option<Triple>,
) -> Result<(), Diagnostic> {
    let refuse = |why: String| {
        Diagnostic::builder(UPDATE_002, Severity::Error)
            .why(why)
            .fix(releases_page())
            .build()
    };
    if offered != manifest {
        return Err(refuse(format!(
            "the offered version was {offered:?}, the signed manifest now names {manifest}; \
             check again before installing"
        )));
    }
    if !above_floor(manifest, floor) {
        let floor = floor.map_or_else(|| "unknown".to_owned(), |(a, b, c)| format!("{a}.{b}.{c}"));
        return Err(refuse(format!(
            "{manifest} is not newer than what is installed here ({floor}, counting a version \
             installed earlier whose restart is still pending); nothing is installed"
        )));
    }
    Ok(())
}

/// Ein Ereignis `progress` auf den Strom, ohne zu warten; ist der Strom
/// voll, fällt dieses eine weg.
pub fn send_progress(
    tx: &tokio::sync::mpsc::Sender<Result<v1::UpdateEvent, tonic::Status>>,
    stage: v1::UpdateStage,
    done: u64,
    total: u64,
) {
    let _ = tx.try_send(Ok(v1::UpdateEvent {
        event: Some(v1::update_event::Event::Progress(v1::UpdateProgress {
            stage: stage.into(),
            done_bytes: done,
            total_bytes: total,
        })),
    }));
}

/// Der Rest eines `Apply` nach allen Prüfungen: laden (`fetch`), dann im
/// Hintergrund-Thread entpacken, prüfen und tauschen. Läuft zu Ende, auch
/// wenn der Client geht: Ein halber Tausch wäre schlimmer als keiner.
#[allow(clippy::too_many_arguments)]
pub async fn finish_apply(
    fetch: impl std::future::Future<Output = Result<(), Diagnostic>>,
    install: Install,
    version: String,
    work: Workdir,
    download: PathBuf,
    total: u64,
    tx: tokio::sync::mpsc::Sender<Result<v1::UpdateEvent, tonic::Status>>,
    guard: tokio::sync::OwnedMutexGuard<()>,
    ledger: PathBuf,
) {
    let _guard = guard;
    send_progress(&tx, v1::UpdateStage::Downloading, 0, total);
    if let Err(diagnostic) = fetch.await {
        let _ = tx
            .send(Err(crate::server_stub::diagnostic_to_status(&diagnostic)))
            .await;
        return;
    }
    send_progress(&tx, v1::UpdateStage::Verifying, total, total);
    let installed = {
        let version = version.clone();
        let tx = tx.clone();
        tokio::task::spawn_blocking(move || {
            let result =
                install_reporting(&install, &download, &version, work.path(), &ledger, &|| {
                    send_progress(&tx, v1::UpdateStage::Installing, total, total);
                });
            drop(work);
            result
        })
        .await
    };
    let event = match installed {
        Ok(Ok(app_path)) => Ok(v1::UpdateEvent {
            event: Some(v1::update_event::Event::Installed(v1::UpdateInstalled {
                version,
                app_path: app_path.display().to_string(),
            })),
        }),
        Ok(Err(diagnostic)) => Err(crate::server_stub::diagnostic_to_status(&diagnostic)),
        Err(error) => Err(crate::server_stub::diagnostic_to_status(&install_failed(
            format!("the installation did not finish: {error}"),
        ))),
    };
    let _ = tx.send(event).await;
}

/// `UPDATE_005`: Der Tausch ist gescheitert.
fn install_failed(why: String) -> Diagnostic {
    Diagnostic::builder(UPDATE_005, Severity::Error)
        .why(why)
        .fix(releases_page())
        .build()
}

/// `UPDATE_003`: Was geladen wurde, besteht eine Prüfung nicht.
fn unpacked_refused(why: String) -> Diagnostic {
    Diagnostic::builder(UPDATE_003, Severity::Error)
        .why(why)
        .fix(releases_page())
        .build()
}

/// Prüft, ob dieses macOS mindestens `min_os` ist (`sw_vers`).
///
/// # Errors
///
/// `UPDATE_002`, wenn das Release ein neueres macOS verlangt. Ist die
/// Version des Systems nicht lesbar, wird nicht abgelehnt: Die Signatur des
/// Bündels und der Start sagen dann das Letzte.
pub fn macos_at_least(min_os: &str) -> Result<(), Diagnostic> {
    let Ok(output) = Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
    else {
        return Ok(());
    };
    let running = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if os_version_at_least(&running, min_os) {
        Ok(())
    } else {
        Err(Diagnostic::builder(UPDATE_002, Severity::Error)
            .why(format!(
                "the new release needs macOS {min_os} or newer, this Mac runs {running}"
            ))
            .fix(releases_page())
            .build())
    }
}

/// `running >= wanted`, Teil für Teil als Zahl; ein unlesbarer Teil zählt
/// als 0, ein fehlender auch.
#[must_use]
pub fn os_version_at_least(running: &str, wanted: &str) -> bool {
    let parts = |text: &str| -> Vec<u64> {
        text.split('.')
            .map(|part| part.parse().unwrap_or(0))
            .collect()
    };
    let (have, want) = (parts(running), parts(wanted));
    let len = have.len().max(want.len());
    for index in 0..len {
        let a = have.get(index).copied().unwrap_or(0);
        let b = want.get(index).copied().unwrap_or(0);
        if a != b {
            return a > b;
        }
    }
    true
}

/// Ein privates Arbeitsverzeichnis (`0700`) unter `parent`, das beim Fallen
/// gelöscht wird.
#[derive(Debug)]
pub struct Workdir {
    path: PathBuf,
}

impl Workdir {
    /// Legt `update-<pid>-<nanos>` unter `parent` an.
    ///
    /// # Errors
    ///
    /// `UPDATE_005`, wenn das Verzeichnis nicht angelegt werden kann.
    pub fn create(parent: &Path) -> Result<Self, Diagnostic> {
        use std::os::unix::fs::DirBuilderExt as _;
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path = parent.join(format!("update-{}-{nanos}", std::process::id()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .recursive(false)
            .create(&path)
            .map_err(|err| {
                install_failed(format!("{} could not be created: {err}", path.display()))
            })?;
        Ok(Self { path })
    }

    /// Der Pfad.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for Workdir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Ein Programm mit festen Argumenten, ohne Shell; `Err` mit dem, was es auf
/// stderr schrieb.
fn run(program: &str, args: &[&std::ffi::OsStr]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|err| format!("{program} could not be started: {err}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!(
            "{program} failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Installiert die geladene, schon gegen das Manifest geprüfte Datei
/// `download` für `install`. Blockiert.
///
/// `version` ist die Version des Manifests; `work` das private
/// Arbeitsverzeichnis, `ledger` das Register aus [`record_installed`]: Es
/// wird vor dem Tausch geschrieben, und ohne es wird nicht getauscht. Die
/// Antwort ist der Pfad, den die App neu startet.
///
/// # Errors
///
/// `UPDATE_003`, wenn der Inhalt eine Prüfung nicht besteht; `UPDATE_005`,
/// wenn Entpacken, Kopieren oder Tausch scheitern; `UPDATE_004` für einen
/// Weg, der nicht im Daemon installiert.
pub fn install(
    install: &Install,
    download: &Path,
    version: &str,
    work: &Path,
    ledger: &Path,
) -> Result<PathBuf, Diagnostic> {
    install_reporting(install, download, version, work, ledger, &|| {})
}

/// Wie [`install`]; `installing` läuft unmittelbar vor dem Tausch.
fn install_reporting(
    install: &Install,
    download: &Path,
    version: &str,
    work: &Path,
    ledger: &Path,
    installing: &dyn Fn(),
) -> Result<PathBuf, Diagnostic> {
    match install {
        Install::AppBundle(bundle) => {
            let unpacked = work.join("unpacked");
            run(
                "/usr/bin/ditto",
                &[
                    "-x".as_ref(),
                    "-k".as_ref(),
                    download.as_os_str(),
                    unpacked.as_os_str(),
                ],
            )
            .map_err(|why| install_failed(format!("the zip could not be unpacked: {why}")))?;
            let app = unpacked.join("Humanitl/Humanitl.app");
            if !std::fs::symlink_metadata(&app).is_ok_and(|meta| meta.is_dir()) {
                return Err(unpacked_refused(
                    "the zip does not hold Humanitl/Humanitl.app".to_owned(),
                ));
            }
            verify_bundle(&app, version)?;
            let staged = sibling(bundle, ".new");
            copy_tree(&app, &staged)?;
            // Die Kopie neben dem Ziel ist das, was gleich dort liegt; sie
            // wird noch einmal geprüft, nicht nur das Original im
            // Arbeitsverzeichnis.
            if let Err(diagnostic) = verify_bundle(&staged, version) {
                let _ = std::fs::remove_dir_all(&staged);
                return Err(diagnostic);
            }
            installing();
            swap_recorded(install, version, ledger, bundle, &staged, &|from, to| {
                std::fs::rename(from, to)
            })?;
            Ok(bundle.clone())
        }
        Install::Archive(root) => {
            let unpacked = work.join("unpacked");
            std::fs::create_dir(&unpacked).map_err(|err| {
                install_failed(format!(
                    "{} could not be created: {err}",
                    unpacked.display()
                ))
            })?;
            run(
                "tar",
                &[
                    "-xzf".as_ref(),
                    download.as_os_str(),
                    "--no-same-owner".as_ref(),
                    "-C".as_ref(),
                    unpacked.as_os_str(),
                ],
            )
            .map_err(|why| install_failed(format!("the archive could not be unpacked: {why}")))?;
            let top = verify_archive(&unpacked, version)?;
            let staged = sibling(root, ".new");
            copy_tree(&top, &staged)?;
            installing();
            swap_recorded(install, version, ledger, root, &staged, &|from, to| {
                std::fs::rename(from, to)
            })?;
            Ok(root.join("app/humanitl"))
        }
        Install::AppImage(file) => {
            use std::os::unix::fs::PermissionsExt as _;
            let staged = sibling(file, ".new");
            let _ = std::fs::remove_file(&staged);
            std::fs::copy(download, &staged).map_err(|err| {
                install_failed(format!("{} could not be written: {err}", staged.display()))
            })?;
            std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).map_err(
                |err| {
                    install_failed(format!(
                        "{} could not be made executable: {err}",
                        staged.display()
                    ))
                },
            )?;
            installing();
            swap_recorded(install, version, ledger, file, &staged, &|from, to| {
                std::fs::rename(from, to)
            })?;
            Ok(file.clone())
        }
        Install::Apt | Install::Dnf | Install::Manual(_) => Err(not_here(
            "this installation is not updated by the daemon itself".to_owned(),
            releases_page(),
        )),
    }
}

/// `<target>.suffix` im selben Ordner.
fn sibling(target: &Path, suffix: &str) -> PathBuf {
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(suffix);
    target.with_file_name(name)
}

/// Kopiert einen Baum mit Rechten und Verweisen an einen neuen Pfad.
fn copy_tree(from: &Path, to: &Path) -> Result<(), Diagnostic> {
    if std::fs::symlink_metadata(to).is_ok() {
        std::fs::remove_dir_all(to).map_err(|err| {
            install_failed(format!(
                "the leftover {} could not be removed: {err}",
                to.display()
            ))
        })?;
    }
    let program = if cfg!(target_os = "macos") {
        "/usr/bin/ditto"
    } else {
        "cp"
    };
    let args: Vec<&std::ffi::OsStr> = if cfg!(target_os = "macos") {
        vec![from.as_os_str(), to.as_os_str()]
    } else {
        vec!["-a".as_ref(), from.as_os_str(), to.as_os_str()]
    };
    run(program, &args).map_err(|why| {
        let _ = std::fs::remove_dir_all(to);
        install_failed(format!(
            "the new version could not be copied next to the old one: {why}"
        ))
    })?;
    Ok(())
}

/// Prüft ein entpacktes Archiv: genau ein Ordner
/// `humanitl-<version>-linux-x86_64` mit den drei Programmen und der App.
fn verify_archive(unpacked: &Path, version: &str) -> Result<PathBuf, Diagnostic> {
    let expected = format!("humanitl-{version}-linux-x86_64");
    let names: Vec<String> = std::fs::read_dir(unpacked)
        .map_err(|err| install_failed(format!("the unpacked archive cannot be read: {err}")))?
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    if names != [expected.clone()] {
        return Err(unpacked_refused(format!(
            "the archive should hold exactly {expected}/, it holds {names:?}"
        )));
    }
    let top = unpacked.join(&expected);
    for file in [
        "bin/humanitld",
        "bin/humanitl",
        "bin/humanitl-shim",
        "app/humanitl",
    ] {
        let ok = std::fs::symlink_metadata(top.join(file)).is_ok_and(|meta| meta.is_file());
        if !ok {
            return Err(unpacked_refused(format!(
                "the archive lacks {expected}/{file}"
            )));
        }
    }
    Ok(top)
}

/// Prüft ein Bündel wie `packaging/macos/build-app.sh`: Signatur,
/// Entitlement für den Ordnerdialog, Version.
/// `CFBundleShortVersionString` eines Bündels; `None`, wenn unlesbar.
fn bundle_version(app: &Path) -> Option<String> {
    let plist = app.join("Contents/Info.plist");
    run(
        "/usr/bin/plutil",
        &[
            "-extract".as_ref(),
            "CFBundleShortVersionString".as_ref(),
            "raw".as_ref(),
            "-o".as_ref(),
            "-".as_ref(),
            plist.as_os_str(),
        ],
    )
    .ok()
    .map(|text| text.trim().to_owned())
}

fn verify_bundle(app: &Path, version: &str) -> Result<(), Diagnostic> {
    run(
        "/usr/bin/codesign",
        &[
            "--verify".as_ref(),
            "--deep".as_ref(),
            "--strict".as_ref(),
            app.as_os_str(),
        ],
    )
    .map_err(|why| unpacked_refused(format!("the new app fails its signature check: {why}")))?;
    let entitlements = run(
        "/usr/bin/codesign",
        &[
            "-d".as_ref(),
            "--entitlements".as_ref(),
            "-".as_ref(),
            "--xml".as_ref(),
            app.as_os_str(),
        ],
    )
    .unwrap_or_default();
    if !entitlements.contains("com.apple.security.files.user-selected.read-only") {
        return Err(unpacked_refused(
            "the new app lacks the entitlement com.apple.security.files.user-selected.read-only"
                .to_owned(),
        ));
    }
    let plist = app.join("Contents/Info.plist");
    let found = run(
        "/usr/bin/plutil",
        &[
            "-extract".as_ref(),
            "CFBundleShortVersionString".as_ref(),
            "raw".as_ref(),
            "-o".as_ref(),
            "-".as_ref(),
            plist.as_os_str(),
        ],
    )
    .map_err(|why| unpacked_refused(format!("the version of the new app cannot be read: {why}")))?;
    if found.trim() != version {
        return Err(unpacked_refused(format!(
            "the new app is version {}, the signed manifest names {version}",
            found.trim()
        )));
    }
    Ok(())
}

/// Tauscht `target` gegen `staged`: `target` wird `.previous`, `staged`
/// wird `target`, und erst dann fällt `.previous`. Scheitert der zweite
/// Schritt, kommt `.previous` zurück.
///
/// `rename` ist im Daemon [`std::fs::rename`]; Tests setzen eine Fassung ein,
/// die an einer bestimmten Stelle scheitert.
///
/// # Errors
///
/// `UPDATE_005` mit dem Schritt und, wenn selbst die Rückkehr scheitert, dem
/// Pfad, unter dem die alte Fassung liegt.
pub fn swap(
    target: &Path,
    staged: &Path,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<(), Diagnostic> {
    let previous = sibling(target, ".previous");
    if let Err(diagnostic) = recover(target, rename) {
        remove_any(staged);
        return Err(diagnostic);
    }
    if let Err(err) = rename(target, &previous) {
        remove_any(staged);
        return Err(install_failed(format!(
            "{} could not be moved aside: {err}; nothing was changed",
            target.display()
        )));
    }
    if let Err(err) = rename(staged, target) {
        remove_any(staged);
        return Err(match rename(&previous, target) {
            Ok(()) => install_failed(format!(
                "the new version could not be moved to {}: {err}; the previous version was \
                 restored",
                target.display()
            )),
            Err(back) => install_failed(format!(
                "the new version could not be moved to {}: {err}; restoring the previous \
                 version failed too ({back}), it lies at {}",
                target.display(),
                previous.display()
            )),
        });
    }
    remove_any(&previous);
    Ok(())
}

/// Räumt vor einem Tausch auf, was ein früherer hinterlassen hat.
///
/// Fehlt das Ziel und liegt `.previous` da, ist ein Tausch mittendrin
/// stehen geblieben: `.previous` kommt zurück an seinen Platz, und gelingt
/// das nicht, bricht alles ab. `.previous` wird nie gelöscht, solange das
/// Ziel fehlt; nur neben einem vorhandenen Ziel ist es ein Rest.
///
/// # Errors
///
/// `UPDATE_005`, wenn das Ziel fehlt und nicht zurückgeholt werden kann,
/// oder wenn ein Rest neben dem Ziel nicht zu entfernen ist.
pub fn recover(
    target: &Path,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<(), Diagnostic> {
    let previous = sibling(target, ".previous");
    let target_there = std::fs::symlink_metadata(target).is_ok();
    let previous_there = std::fs::symlink_metadata(&previous).is_ok();
    match (target_there, previous_there) {
        (true, false) => Ok(()),
        (false, true) => rename(&previous, target).map_err(|err| {
            install_failed(format!(
                "an earlier update stopped halfway: {} is missing and the previous version lies \
                 at {}; moving it back failed ({err}), so nothing was changed. Move it back by \
                 hand",
                target.display(),
                previous.display()
            ))
        }),
        (false, false) => Err(install_failed(format!(
            "{} does not exist, so there is nothing to replace",
            target.display()
        ))),
        (true, true) => {
            let removed = if previous.is_dir() && !previous.is_symlink() {
                std::fs::remove_dir_all(&previous)
            } else {
                std::fs::remove_file(&previous)
            };
            removed.map_err(|err| {
                install_failed(format!(
                    "the leftover {} could not be removed: {err}",
                    previous.display()
                ))
            })
        }
    }
}

/// Vermerkt `version` im Register und tauscht dann (Review zu HUM-382).
///
/// Das Register ist die Untergrenze gegen ein Downgrade, solange der
/// Neustart nach einem Update aussteht. Ein Tausch ohne Eintrag ließe genau
/// dieses Downgrade zu; deshalb wird zuerst geschrieben, und kann das
/// Register nicht geschrieben werden, wird nicht getauscht. Scheitert danach
/// der Tausch, bekommt das Register seinen alten Inhalt zurück.
///
/// # Errors
///
/// `UPDATE_005`, wenn das Register nicht geschrieben werden kann oder der
/// Tausch scheitert ([`swap`]).
pub fn swap_recorded(
    install: &Install,
    version: &str,
    ledger: &Path,
    target: &Path,
    staged: &Path,
    rename: &dyn Fn(&Path, &Path) -> io::Result<()>,
) -> Result<(), Diagnostic> {
    let before = std::fs::read(ledger).ok();
    if let Err(diagnostic) = record_installed(ledger, install, version) {
        remove_any(staged);
        return Err(install_failed(format!(
            "{}; without that record an older release could be installed again before the \
             restart, so nothing was changed",
            diagnostic.why
        )));
    }
    swap(target, staged, rename).inspect_err(|_| {
        let restored = match &before {
            Some(bytes) => std::fs::write(ledger, bytes),
            None => std::fs::remove_file(ledger),
        };
        if let Err(err) = restored {
            tracing::warn!(%err, ledger = %ledger.display(), "the update record could not be reset");
        }
    })
}

/// Holt einen stehen gebliebenen Tausch zurück, bevor irgendetwas anderes
/// geschieht: vor der Erkennung des Wegs, vor jeder Anfrage ins Netz
/// (Review zu HUM-382). Das Ziel muss dafür nicht existieren; es folgt aus
/// dem Pfad der laufenden Installation (`exe`, beim Start des Daemons
/// gemerkt) und dem `AppImage` der App:
///
/// - unter macOS das Bündel über `exe`;
/// - unter Linux der Archiv-Ordner `<root>` über `<root>/bin/humanitld`,
///   wenn `<root>.previous/bin/humanitld` daliegt;
/// - das `AppImage`, wenn sein Name auf `.AppImage` endet und
///   `<datei>.previous` eine Datei ist.
///
/// Gehandelt wird nur im Zustand „Ziel fehlt, `.previous` da"; alles andere
/// bleibt, wie es ist.
///
/// # Errors
///
/// `UPDATE_005`, wenn `.previous` nicht zurückgeholt werden kann.
pub fn recover_local(exe: &Path, appimage: Option<&Path>, host: &Host) -> Result<(), Diagnostic> {
    let mut targets: Vec<PathBuf> = Vec::new();
    if host.macos {
        if let Ok(bundle) = bundle_of(exe) {
            targets.push(bundle);
        }
    } else {
        if let Some(image) = appimage
            && image.is_absolute()
            && image
                .file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".AppImage"))
            && std::fs::symlink_metadata(sibling(image, ".previous")).is_ok_and(|m| m.is_file())
        {
            targets.push(image.to_path_buf());
        }
        if let Some(root) = exe
            .parent()
            .filter(|bin| bin.file_name().is_some_and(|name| name == "bin"))
            .and_then(Path::parent)
            && !root.starts_with("/usr")
            && sibling(root, ".previous").join("bin/humanitld").is_file()
        {
            targets.push(root.to_path_buf());
        }
    }
    for target in targets {
        let missing = std::fs::symlink_metadata(&target).is_err();
        let previous = std::fs::symlink_metadata(sibling(&target, ".previous")).is_ok();
        if missing && previous {
            recover(&target, &|from, to| std::fs::rename(from, to))?;
            tracing::info!(target = %target.display(), "restored the previous version of an interrupted update");
        }
    }
    Ok(())
}

/// [`recover`] für das Ziel dieses Wegs; ohne Ziel nichts.
///
/// # Errors
///
/// Wie [`recover`].
pub fn recover_target(install: &Install) -> Result<(), Diagnostic> {
    target_of(install).map_or(Ok(()), |target| {
        recover(target, &|from, to| std::fs::rename(from, to))
    })
}

fn remove_any(path: &Path) {
    let _ = if path.is_dir() && !path.is_symlink() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::cell::Cell;
    use std::fs;

    use super::*;

    fn linux(root: &Path) -> Host {
        Host {
            root: root.to_path_buf(),
            macos: false,
        }
    }

    fn touch(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn a_deb_from_the_apt_repository_with_pkexec_is_apt() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        touch(&root.join("var/lib/dpkg/info/humanitl.list"), "");
        touch(
            &root.join("etc/apt/sources.list.d/nurkert.list"),
            "deb [signed-by=/usr/share/keyrings/nurkert-archive-keyring.gpg] \
             https://apt.nurkert.de stable main\n",
        );
        touch(&root.join("usr/bin/pkexec"), "");
        let exe = Path::new("/usr/lib/humanitl/bin/humanitld");
        let found = detect(exe, None, &linux(root));
        assert_eq!(found, Install::Apt);
        assert_eq!(
            found.upgrade_command(),
            [
                "pkexec",
                "apt-get",
                "install",
                "--only-upgrade",
                "-y",
                "humanitl"
            ]
        );

        // Ohne pkexec: der Befehl zum Kopieren.
        fs::remove_file(root.join("usr/bin/pkexec")).unwrap();
        let Install::Manual(diagnostic) = detect(exe, None, &linux(root)) else {
            panic!("expected manual");
        };
        assert_eq!(diagnostic.code, UPDATE_004);
        assert_eq!(
            diagnostic.fix,
            Some(FixAction::CopyCommand(
                "sudo apt-get install --only-upgrade humanitl".to_owned()
            ))
        );

        // Ohne Repository: die Release-Seite.
        fs::remove_file(root.join("etc/apt/sources.list.d/nurkert.list")).unwrap();
        let Install::Manual(diagnostic) = detect(exe, None, &linux(root)) else {
            panic!("expected manual");
        };
        assert_eq!(diagnostic.fix, Some(releases_page()));

        // Eine auskommentierte Quelle ist keine.
        touch(&root.join("usr/bin/pkexec"), "");
        touch(
            &root.join("etc/apt/sources.list.d/nurkert.list"),
            "# deb https://apt.nurkert.de stable main\n",
        );
        assert!(matches!(
            detect(exe, None, &linux(root)),
            Install::Manual(_)
        ));

        // Die geladene Paketliste des Repositorys genügt auch, etwa wenn die
        // Quelle in einer deb822-Datei unter anderem Namen steht.
        touch(
            &root.join("var/lib/apt/lists/apt.nurkert.de_dists_stable_main_binary-amd64_Packages"),
            "Package: humanitl\nVersion: 0.0.5\n",
        );
        assert_eq!(detect(exe, None, &linux(root)), Install::Apt);
    }

    #[test]
    fn an_rpm_from_the_dnf_repository_is_dnf() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("var/lib/rpm")).unwrap();
        touch(&root.join("etc/yum.repos.d/humanitl.repo"), "[humanitl]\n");
        touch(&root.join("usr/bin/pkexec"), "");
        let found = detect(
            Path::new("/usr/lib/humanitl/bin/humanitld"),
            None,
            &linux(root),
        );
        assert_eq!(found, Install::Dnf);
        assert_eq!(
            found.upgrade_command(),
            ["pkexec", "dnf", "upgrade", "-y", "humanitl"]
        );
        assert!(!found.applies_in_daemon());
    }

    #[test]
    fn the_aur_is_never_run_only_shown() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("var/lib/pacman/local/humanitl-bin-0.0.5-1")).unwrap();
        touch(&root.join("usr/bin/pkexec"), "");
        let Install::Manual(diagnostic) = detect(
            Path::new("/usr/lib/humanitl/bin/humanitld"),
            None,
            &linux(root),
        ) else {
            panic!("expected manual");
        };
        assert!(matches!(diagnostic.fix, Some(FixAction::CopyCommand(_))));
        assert!(diagnostic.why.contains("AUR"));
    }

    #[test]
    fn an_unpacked_archive_in_a_writable_folder_is_archive() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("humanitl-0.0.5-linux-x86_64");
        touch(&top.join("bin/humanitld"), "");
        fs::create_dir_all(top.join("app")).unwrap();
        fs::create_dir_all(top.join("share/humanitl/catalog")).unwrap();
        let found = detect(&top.join("bin/humanitld"), None, &linux(dir.path()));
        assert_eq!(found, Install::Archive(top.clone()));
        assert_eq!(found.manifest(), "update-linux-x86_64.json");

        // Ein Baum ohne `app/` ist kein Release-Archiv, etwa ein Build.
        fs::remove_dir_all(top.join("app")).unwrap();
        assert!(matches!(
            detect(&top.join("bin/humanitld"), None, &linux(dir.path())),
            Install::Manual(_)
        ));
    }

    #[test]
    fn an_appimage_is_checked_before_it_is_trusted() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("Humanitl-0.0.5-x86_64.AppImage");
        touch(&image, "elf");
        let exe = Path::new("/home/u/.local/lib/humanitl/current/bin/humanitld");
        let found = detect(exe, Some(&image), &linux(dir.path()));
        assert_eq!(found, Install::AppImage(image.clone()));
        assert_eq!(found.manifest(), "update-linux-x86_64-appimage.json");

        for bad in [
            PathBuf::from("relative.AppImage"),
            PathBuf::from("/usr/bin/Humanitl.AppImage"),
            dir.path().join("not-an-image"),
            dir.path().join("missing.AppImage"),
        ] {
            assert!(
                matches!(
                    detect(exe, Some(&bad), &linux(dir.path())),
                    Install::Manual(_)
                ),
                "{}",
                bad.display()
            );
        }
        // Die Kopie des AppImages ohne Pfad: kein Weg aus dem Daemon.
        assert!(matches!(
            detect(exe, None, &linux(dir.path())),
            Install::Manual(_)
        ));
    }

    #[test]
    fn a_bundle_on_a_disk_image_or_translocated_is_refused() {
        let ok = bundle_of(Path::new(
            "/Applications/Humanitl.app/Contents/MacOS/bin/humanitld",
        ));
        assert_eq!(ok.unwrap(), PathBuf::from("/Applications/Humanitl.app"));
        for bad in [
            "/Volumes/Humanitl/Humanitl.app/Contents/MacOS/bin/humanitld",
            "/private/var/folders/x/AppTranslocation/ABC/d/Humanitl.app/Contents/MacOS/bin/humanitld",
            "/Users/u/src/humanitl/daemon/target/debug/humanitld",
        ] {
            assert_eq!(
                bundle_of(Path::new(bad)).unwrap_err().code,
                UPDATE_004,
                "{bad}"
            );
        }
    }

    #[test]
    fn the_updater_asks_at_most_once_a_day_and_says_what_it_knows() {
        let updater = Updater::new("0.0.5");
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000_000);
        assert!(updater.due(now));
        updater.attempted(now);
        assert!(!updater.due(now + Duration::from_secs(3600)));
        assert!(updater.due(now + CHECK_INTERVAL));

        let status = updater.status(&Install::Archive(PathBuf::from("/x")));
        assert!(!status.available);
        assert!(status.latest_version.is_empty());

        updater.record(
            Offer {
                version: "0.0.6".to_owned(),
                newer: true,
                size: 10,
            },
            now,
        );
        let status = updater.status(&Install::Apt);
        assert!(status.available && status.can_apply);
        assert_eq!(status.method, i32::from(v1::UpdateMethod::Apt));
        assert_eq!(status.upgrade_command, apt_command());
        let manual = Install::Manual(Box::new(not_here("x".to_owned(), releases_page())));
        let status = updater.status(&manual);
        assert!(status.available && !status.can_apply);
        assert_eq!(status.notice.unwrap().code, "UPDATE_004");
    }

    #[test]
    fn os_versions_compare_by_number() {
        assert!(os_version_at_least("14.5", "13.0"));
        assert!(os_version_at_least("13.0", "13"));
        assert!(os_version_at_least("10.15.7", "10.15"));
        assert!(!os_version_at_least("12.7.4", "13.0"));
        assert!(!os_version_at_least("13.9", "13.10"));
    }

    fn tree(root: &Path, marker: &str) {
        touch(&root.join("bin/humanitld"), marker);
    }

    #[test]
    fn a_swap_replaces_the_tree_and_removes_the_previous_one() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("Humanitl.app");
        let staged = home.path().join("Humanitl.app.new");
        tree(&target, "old");
        tree(&staged, "new");
        swap(&target, &staged, &|a, b| fs::rename(a, b)).unwrap();
        assert_eq!(
            fs::read_to_string(target.join("bin/humanitld")).unwrap(),
            "new"
        );
        assert!(!home.path().join("Humanitl.app.previous").exists());
        assert!(!staged.exists());
    }

    #[test]
    fn a_failed_swap_restores_the_previous_tree() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("Humanitl.app");
        let staged = home.path().join("Humanitl.app.new");
        tree(&target, "old");
        tree(&staged, "new");
        // Der zweite Schritt (neu an seinen Platz) scheitert.
        let calls = Cell::new(0);
        let refused = swap(&target, &staged, &|a, b| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                Err(io::Error::other("disk full"))
            } else {
                fs::rename(a, b)
            }
        })
        .unwrap_err();
        assert_eq!(refused.code, UPDATE_005);
        assert!(refused.why.contains("restored"), "{}", refused.why);
        assert_eq!(
            fs::read_to_string(target.join("bin/humanitld")).unwrap(),
            "old"
        );
        assert!(!staged.exists());
        assert!(!home.path().join("Humanitl.app.previous").exists());

        // Scheitert schon das Beiseitelegen, bleibt alles, wie es war.
        tree(&staged, "new");
        let refused = swap(&target, &staged, &|_, _| Err(io::Error::other("busy"))).unwrap_err();
        assert!(
            refused.why.contains("nothing was changed"),
            "{}",
            refused.why
        );
        assert_eq!(
            fs::read_to_string(target.join("bin/humanitld")).unwrap(),
            "old"
        );
    }

    #[test]
    fn an_archive_with_the_wrong_layout_or_version_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let top = dir.path().join("humanitl-0.0.6-linux-x86_64");
        for file in [
            "bin/humanitld",
            "bin/humanitl",
            "bin/humanitl-shim",
            "app/humanitl",
        ] {
            touch(&top.join(file), "");
        }
        assert_eq!(verify_archive(dir.path(), "0.0.6").unwrap(), top);
        assert_eq!(
            verify_archive(dir.path(), "0.0.7").unwrap_err().code,
            UPDATE_003
        );
        fs::remove_file(top.join("bin/humanitl-shim")).unwrap();
        assert_eq!(
            verify_archive(dir.path(), "0.0.6").unwrap_err().code,
            UPDATE_003
        );
    }

    /// Der ganze Weg eines Archivs unter einem Wegwerf-HOME: entpacken,
    /// prüfen, tauschen. `tar` des Systems, keine Netzverbindung.
    #[test]
    fn an_archive_update_replaces_the_folder() {
        let home = tempfile::tempdir().unwrap();
        let build = home.path().join("build");
        let new_top = build.join("humanitl-0.0.6-linux-x86_64");
        for file in [
            "bin/humanitld",
            "bin/humanitl",
            "bin/humanitl-shim",
            "app/humanitl",
        ] {
            touch(&new_top.join(file), "new");
        }
        let archive = home.path().join("update.tar.gz");
        run(
            "tar",
            &[
                "-czf".as_ref(),
                archive.as_os_str(),
                "-C".as_ref(),
                build.as_os_str(),
                "humanitl-0.0.6-linux-x86_64".as_ref(),
            ],
        )
        .unwrap();
        let root = home.path().join("humanitl-0.0.5-linux-x86_64");
        touch(&root.join("bin/humanitld"), "old");
        let work = Workdir::create(home.path()).unwrap();
        let relaunch = install(
            &Install::Archive(root.clone()),
            &archive,
            "0.0.6",
            work.path(),
            &home.path().join("ledger.json"),
        )
        .unwrap();
        assert_eq!(relaunch, root.join("app/humanitl"));
        assert_eq!(
            installed_floor(
                &Install::Archive(root.clone()),
                "0.0.1",
                &home.path().join("ledger.json")
            ),
            Some((0, 0, 6))
        );
        assert_eq!(
            fs::read_to_string(root.join("bin/humanitld")).unwrap(),
            "new"
        );
        assert!(
            !home
                .path()
                .join("humanitl-0.0.5-linux-x86_64.previous")
                .exists()
        );

        // Dieselbe Datei als 0.0.7 ausgegeben: abgelehnt, nichts getauscht.
        let work = Workdir::create(home.path()).unwrap();
        let refused = install(
            &Install::Archive(root.clone()),
            &archive,
            "0.0.7",
            work.path(),
            &home.path().join("ledger.json"),
        )
        .unwrap_err();
        assert_eq!(refused.code, UPDATE_003);
        assert_eq!(
            fs::read_to_string(root.join("bin/humanitld")).unwrap(),
            "new"
        );
    }

    #[test]
    fn an_appimage_update_replaces_the_file_executable() {
        use std::os::unix::fs::PermissionsExt as _;
        let home = tempfile::tempdir().unwrap();
        let image = home.path().join("Humanitl.AppImage");
        touch(&image, "old");
        let download = home.path().join("download");
        touch(&download, "new");
        let work = Workdir::create(home.path()).unwrap();
        let path = install(
            &Install::AppImage(image.clone()),
            &download,
            "0.0.6",
            work.path(),
            &home.path().join("ledger.json"),
        )
        .unwrap();
        assert_eq!(path, image);
        assert_eq!(fs::read_to_string(&image).unwrap(), "new");
        assert_eq!(
            fs::metadata(&image).unwrap().permissions().mode() & 0o777,
            0o755
        );
    }

    #[test]
    fn the_workdir_is_private_and_goes_away() {
        use std::os::unix::fs::PermissionsExt as _;
        let parent = tempfile::tempdir().unwrap();
        let work = Workdir::create(parent.path()).unwrap();
        let path = work.path().to_path_buf();
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o700
        );
        drop(work);
        assert!(!path.exists());
    }

    /// Ein ad-hoc signiertes Bündel unter einem Wegwerf-HOME, geprüft wie
    /// ein echtes; nur unter macOS, wo `codesign` liegt.
    #[cfg(target_os = "macos")]
    #[test]
    fn a_bundle_is_checked_like_the_release_build_does() {
        let home = tempfile::tempdir().unwrap();
        let app = home.path().join("Humanitl.app");
        // Ein echtes Mach-O: Entitlements stehen in seiner Signatur.
        fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        fs::copy("/usr/bin/true", app.join("Contents/MacOS/Humanitl")).unwrap();
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(
                app.join("Contents/MacOS/Humanitl"),
                fs::Permissions::from_mode(0o755),
            )
            .unwrap();
        }
        touch(
            &app.join("Contents/Info.plist"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\
             <key>CFBundleExecutable</key><string>Humanitl</string>\
             <key>CFBundleIdentifier</key><string>dev.humanitl.test</string>\
             <key>CFBundleShortVersionString</key><string>0.0.6</string>\
             </dict></plist>\n",
        );
        let ents = home.path().join("ents.plist");
        touch(
            &ents,
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict>\
             <key>com.apple.security.files.user-selected.read-only</key><true/></dict></plist>\n",
        );
        // Ohne Signatur: abgelehnt.
        assert_eq!(verify_bundle(&app, "0.0.6").unwrap_err().code, UPDATE_003);
        run(
            "/usr/bin/codesign",
            &[
                "--force".as_ref(),
                "--sign".as_ref(),
                "-".as_ref(),
                "--entitlements".as_ref(),
                ents.as_os_str(),
                app.as_os_str(),
            ],
        )
        .unwrap();
        verify_bundle(&app, "0.0.6").unwrap();
        // Die Untergrenze liest die Version des installierten Bündels.
        let ledger = home.path().join("ledger.json");
        assert_eq!(
            installed_floor(&Install::AppBundle(app.clone()), "0.0.1", &ledger),
            Some((0, 0, 6))
        );
        // Falsche Version: abgelehnt.
        assert!(
            verify_bundle(&app, "0.0.7")
                .unwrap_err()
                .why
                .contains("0.0.7")
        );
    }

    #[test]
    fn the_floor_counts_a_version_installed_but_not_yet_running() {
        let dir = tempfile::tempdir().unwrap();
        let ledger = dir.path().join("data/update-installed.json");
        let install = Install::Archive(dir.path().join("humanitl-0.0.5-linux-x86_64"));
        // Ohne Register gilt die Version des Daemons.
        let floor = installed_floor(&install, "0.0.5", &ledger);
        assert_eq!(floor, Some((0, 0, 5)));
        assert!(above_floor("0.0.6", floor));

        // 0.0.7 wurde installiert, der Daemon läuft noch als 0.0.5.
        record_installed(&ledger, &install, "0.0.7").unwrap();
        let floor = installed_floor(&install, "0.0.5", &ledger);
        assert_eq!(floor, Some((0, 0, 7)));
        let refused = offer_matches("0.0.6", "0.0.6", floor).unwrap_err();
        assert_eq!(refused.code, UPDATE_002);
        assert!(offer_matches("0.0.7", "0.0.7", floor).is_err());
        offer_matches("0.0.8", "0.0.8", floor).unwrap();

        // Das Register sinkt nie, und es gilt nur für sein Ziel.
        record_installed(&ledger, &install, "0.0.6").unwrap();
        assert_eq!(installed_floor(&install, "0.0.5", &ledger), Some((0, 0, 7)));
        let other = Install::AppImage(dir.path().join("Other.AppImage"));
        assert_eq!(installed_floor(&other, "0.0.5", &ledger), Some((0, 0, 5)));
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(
                fs::metadata(&ledger).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[test]
    fn versions_are_read_strictly() {
        assert_eq!(parse_version("v0.0.12"), Some((0, 0, 12)));
        for bad in ["", "0.0", "0.0.0.1", "0.0.01", "0.0.x", "0.0.1-rc"] {
            assert_eq!(parse_version(bad), None, "{bad:?}");
        }
        assert!(!above_floor("garbage", None));
    }

    #[test]
    fn an_interrupted_swap_is_restored_before_the_next_one() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("Humanitl.app");
        let previous = home.path().join("Humanitl.app.previous");
        let staged = home.path().join("Humanitl.app.new");
        // Ein früherer Tausch blieb stehen: Ziel fehlt, die alte Fassung liegt
        // als `.previous` da.
        tree(&previous, "old");
        tree(&staged, "new");
        swap(&target, &staged, &|a, b| fs::rename(a, b)).unwrap();
        assert_eq!(
            fs::read_to_string(target.join("bin/humanitld")).unwrap(),
            "new"
        );
        assert!(!previous.exists());
    }

    #[test]
    fn previous_is_never_deleted_while_the_target_is_missing() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("Humanitl.app");
        let previous = home.path().join("Humanitl.app.previous");
        let staged = home.path().join("Humanitl.app.new");
        tree(&previous, "old");
        tree(&staged, "new");
        // Auch das Zurückholen scheitert: abgelehnt, `.previous` bleibt.
        let refused = swap(&target, &staged, &|_, _| Err(io::Error::other("busy"))).unwrap_err();
        assert_eq!(refused.code, UPDATE_005);
        assert!(refused.why.contains("stopped halfway"), "{}", refused.why);
        assert_eq!(
            fs::read_to_string(previous.join("bin/humanitld")).unwrap(),
            "old"
        );
        assert!(!target.exists());

        // `recover_target` holt es zurück, bevor ein Apply die Version liest.
        recover_target(&Install::AppBundle(target.clone())).unwrap();
        assert_eq!(
            fs::read_to_string(target.join("bin/humanitld")).unwrap(),
            "old"
        );
        assert!(!previous.exists());

        // Weder Ziel noch `.previous`: nichts zu ersetzen.
        fs::remove_dir_all(&target).unwrap();
        assert_eq!(
            recover_target(&Install::AppBundle(target))
                .unwrap_err()
                .code,
            UPDATE_005
        );
    }

    #[test]
    fn no_swap_without_a_record_of_the_version() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("Humanitl.AppImage");
        let staged = home.path().join("Humanitl.AppImage.new");
        touch(&target, "old");
        touch(&staged, "new");
        // Ein Register, das nicht geschrieben werden kann: Sein Ordner ist eine
        // Datei.
        touch(&home.path().join("data"), "");
        let ledger = home.path().join("data/update-installed.json");
        let install = Install::AppImage(target.clone());
        let refused = swap_recorded(&install, "0.0.7", &ledger, &target, &staged, &|a, b| {
            fs::rename(a, b)
        })
        .unwrap_err();
        assert_eq!(refused.code, UPDATE_005);
        assert!(
            refused.why.contains("nothing was changed"),
            "{}",
            refused.why
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");
        assert!(!staged.exists());
    }

    #[test]
    fn a_failed_swap_resets_the_record() {
        let home = tempfile::tempdir().unwrap();
        let target = home.path().join("Humanitl.AppImage");
        let staged = home.path().join("Humanitl.AppImage.new");
        touch(&target, "old");
        touch(&staged, "new");
        let ledger = home.path().join("ledger.json");
        let install = Install::AppImage(target.clone());
        record_installed(&ledger, &install, "0.0.6").unwrap();
        let calls = Cell::new(0);
        swap_recorded(&install, "0.0.7", &ledger, &target, &staged, &|a, b| {
            calls.set(calls.get() + 1);
            if calls.get() == 2 {
                Err(io::Error::other("disk full"))
            } else {
                fs::rename(a, b)
            }
        })
        .unwrap_err();
        assert_eq!(installed_floor(&install, "0.0.1", &ledger), Some((0, 0, 6)));
        assert_eq!(fs::read_to_string(&target).unwrap(), "old");

        // Gelingt der Tausch, steht die neue Version im Register.
        touch(&staged, "new");
        swap_recorded(&install, "0.0.7", &ledger, &target, &staged, &|a, b| {
            fs::rename(a, b)
        })
        .unwrap();
        assert_eq!(installed_floor(&install, "0.0.1", &ledger), Some((0, 0, 7)));
    }

    #[test]
    fn an_interrupted_swap_is_recovered_without_the_target() {
        let home = tempfile::tempdir().unwrap();
        // Archiv: `<root>` fehlt, `<root>.previous` trägt die alte Fassung.
        let root = home.path().join("humanitl-0.0.5-linux-x86_64");
        let exe = root.join("bin/humanitld");
        touch(
            &home
                .path()
                .join("humanitl-0.0.5-linux-x86_64.previous/bin/humanitld"),
            "old",
        );
        // AppImage ebenso.
        let image = home.path().join("Humanitl.AppImage");
        touch(&home.path().join("Humanitl.AppImage.previous"), "old image");
        recover_local(&exe, Some(&image), &linux(home.path())).unwrap();
        assert_eq!(fs::read_to_string(&exe).unwrap(), "old");
        assert_eq!(fs::read_to_string(&image).unwrap(), "old image");
        // Danach erkennt `detect` das AppImage wieder.
        assert_eq!(
            detect(&exe, Some(&image), &linux(home.path())),
            Install::AppImage(image.clone())
        );
        // Ein zweiter Aufruf ändert nichts.
        recover_local(&exe, Some(&image), &linux(home.path())).unwrap();
        assert_eq!(fs::read_to_string(&image).unwrap(), "old image");

        // macOS: das Bündel über dem Pfad des Daemons.
        let bundle = home.path().join("Humanitl.app");
        touch(
            &home
                .path()
                .join("Humanitl.app.previous/Contents/Info.plist"),
            "x",
        );
        let mac = Host {
            root: home.path().to_path_buf(),
            macos: true,
        };
        recover_local(&bundle.join("Contents/MacOS/bin/humanitld"), None, &mac).unwrap();
        assert!(bundle.join("Contents/Info.plist").is_file());
    }

    #[test]
    fn only_a_recorded_appimage_path_is_trusted() {
        use std::os::unix::fs::PermissionsExt as _;
        let home = tempfile::tempdir().unwrap();
        let record = home.path().join("appimage-path");
        let ledger = home.path().join("ledger.json");
        let image = home.path().join("Apps/Humanitl.AppImage");
        let unrelated = home.path().join("other/Victim.AppImage");
        assert!(!appimage_trusted(&image, &record, &ledger));
        touch(&record, &format!("{}\n", image.display()));
        fs::set_permissions(&record, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(appimage_trusted(&image, &record, &ledger));
        assert!(!appimage_trusted(&unrelated, &record, &ledger));
        // Eine Aufzeichnung, die andere schreiben dürfen, zählt nicht.
        fs::set_permissions(&record, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(!appimage_trusted(&image, &record, &ledger));
        // Ein Ziel aus dem Register des Daemons zählt.
        record_installed(&ledger, &Install::AppImage(unrelated.clone()), "0.0.6").unwrap();
        assert!(appimage_trusted(&unrelated, &record, &ledger));
    }
}
