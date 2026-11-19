//! Das Verzeichnis je Instanz unter dem Laufzeitverzeichnis des Nutzers,
//! `humanitl-vm-<Instanz>`, und wer es wann wegräumen darf (HUM-492).
//!
//! **Belegt heißt gesperrt.** Ein Prozess, der darin eine Sitzung anlegt,
//! hält vorher eine geteilte Sperre (`flock(LOCK_SH)`) auf die Datei
//! [`LOCK_FILE`] darin ([`enter`]), bis seine letzte Sitzung weg ist. Die
//! Sperre vergeht mit dem Prozess, auch nach `SIGKILL`. Wer eine exklusive
//! Sperre bekommt, hat damit bewiesen, dass kein lebender Prozess das
//! Verzeichnis benutzt; nur dann wird es geräumt. „Nicht meins" allein
//! reicht nie: Zwei Daemons (ein Test neben dem echten) haben verschiedene
//! Instanzen unter demselben Laufzeitverzeichnis.
//!
//! **Weg mit der letzten Sitzung.** Endet die letzte Sitzung eines Prozesses
//! ([`session_dir_removed`]), wandelt er die Sperre in eine exklusive; nur
//! wenn das gelingt und außer der Sperrdatei nichts mehr im Verzeichnis
//! liegt, entfernt er Sperrdatei und Verzeichnis. Am Ende des Daemons gibt
//! [`release_all`] jede Sperre frei, die dann noch steht.
//!
//! **Was ein beendeter Prozess liegen ließ**, räumt der Start eines Daemons
//! weg ([`find_orphans`], [`Orphans::remove`]): nur Einträge, die genau
//! `humanitl-vm-` und 16 kleine Hex-Ziffern heißen, Verzeichnisse dieses
//! Nutzers sind, eine Sperrdatei tragen und sich exklusiv sperren lassen.
//! Ein Verzeichnis ohne Sperrdatei (aus der Zeit vor HUM-492, oder eines,
//! das gerade entsteht) bleibt liegen; nur das eigene der Instanz bekommt
//! beim Start eine, denn dass kein zweiter Daemon dieser Instanz läuft, hat
//! der Start schon bewiesen.
//!
//! **Nicht leer.** Ein verwaistes Verzeichnis mit Sitzungen darin wird ganz
//! entfernt: Sein Besitzer ist nachweislich tot, die Units der Sitzungen
//! endeten mit seiner Lebensleine (oder werden vorher gestoppt), und was
//! darin liegt, belegt Arbeitsspeicher (tmpfs) und trägt womöglich den
//! Gastplan mit der Umgebung des Agenten. Jede Sitzung darin ist ein
//! `SANDBOX_041` als Warnung, wie bisher bei der eigenen Instanz. Die Unit,
//! die vorher gestoppt wird, ergibt sich aus dem Namen des Verzeichnisses
//! der Sitzung ([`session_unit`]), nie aus einer Datei darin.
//!
//! **Keine Verweise, keine Einhängungen.** Alles geht über Deskriptoren von
//! Verzeichnissen: `openat2` mit `O_NOFOLLOW`, `O_DIRECTORY` und
//! `RESOLVE_NO_XDEV`, dazu der Vergleich des Geräts mit dem des
//! Laufzeitverzeichnisses, `unlinkat` relativ zum geprüften Verzeichnis. Ein
//! symbolischer Verweis wird als Eintrag entfernt, nie verfolgt; einer
//! anstelle des Verzeichnisses einer Instanz oder der Sperrdatei lässt den
//! Eintrag unberührt. Ein eingehängtes Dateisystem (auch ein Bind-Mount)
//! wird nie betreten; die Sitzung darum bleibt liegen und wird gemeldet,
//! ebenso eine, die tiefer als [`MAX_DEPTH`] Ebenen reicht.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use rustix::fs::{AtFlags, CWD, FileType, FlockOperation, Mode, OFlags, ResolveFlags, Stat};
use rustix::io::Errno;

use crate::user_unit::UnitName;

/// Der Anfang des Namens; danach die Instanz.
pub(crate) const PREFIX: &str = "humanitl-vm-";
/// Die Zahl der Hex-Ziffern der Instanz.
pub(crate) const INSTANCE_HEX: usize = 16;
/// Die Sperrdatei im Verzeichnis der Instanz.
pub(crate) const LOCK_FILE: &str = "lock";
/// Die Zahl der Hex-Ziffern im Namen einer Sitzung (`Uuid::simple`).
const SESSION_HEX: usize = 32;
/// Wie lange [`enter`] darauf wartet, dass ein anderer Daemon eine verwaiste
/// Instanz desselben Namens fertig geräumt hat.
const CLAIM_WAIT: Duration = Duration::from_secs(30);
/// Wie tief das Entfernen in ein verwaistes Verzeichnis hinabsteigt; das
/// Laufzeitverzeichnis einer Sitzung ist drei Ebenen tief. Was tiefer reicht,
/// hat nicht der Daemon angelegt; es bleibt liegen und wird gemeldet
/// ([`Kept::TooDeep`]), statt mit einem Deskriptor je Ebene hinabzusteigen.
pub(crate) const MAX_DEPTH: u32 = 32;
/// Wie viele Namen ein Durchgang über ein Verzeichnis höchstens liest.
const NAMES_PER_PASS: usize = 1024;

/// Was dieser Prozess an Instanzen belegt: je Verzeichnis die geteilte
/// Sperre und die Namen seiner Sitzungen darin.
static CLAIMS: Mutex<BTreeMap<PathBuf, Claim>> = Mutex::new(BTreeMap::new());

/// Die Belegung einer Instanz durch diesen Prozess.
#[derive(Debug)]
struct Claim {
    /// Die Sperrdatei, geteilt gesperrt.
    lock: OwnedFd,
    /// Die Sitzungen dieses Prozesses darin.
    sessions: BTreeSet<OsString>,
}

fn claims() -> MutexGuard<'static, BTreeMap<PathBuf, Claim>> {
    CLAIMS.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Ob `name` genau das Schema einer Instanz trägt.
pub(crate) fn is_instance_name(name: &[u8]) -> bool {
    name.strip_prefix(PREFIX.as_bytes()).is_some_and(|hex| {
        hex.len() == INSTANCE_HEX
            && hex
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
    })
}

fn same_file(a: &Stat, b: &Stat) -> bool {
    a.st_dev == b.st_dev && a.st_ino == b.st_ino
}

fn own_uid() -> u32 {
    rustix::process::getuid().as_raw()
}

/// Öffnet das Verzeichnis `name` unter `parent`, ohne einem Verweis zu
/// folgen.
fn open_dir(parent: BorrowedFd<'_>, name: impl rustix::path::Arg) -> Result<OwnedFd, Errno> {
    rustix::fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
}

/// Öffnet das Verzeichnis `name` unter `parent` für das Räumen: ohne einem
/// Verweis zu folgen und ohne eine Einhängung zu überschreiten
/// (`openat2` mit `RESOLVE_NO_XDEV`, auch für Bind-Mounts desselben
/// Dateisystems), und nur, wenn es auf dem Gerät `dev` liegt. Ein fremdes
/// Dateisystem in einer verwaisten Instanz wird so nie betreten.
fn open_dir_here(parent: BorrowedFd<'_>, name: &OsStr, dev: u64) -> Result<OwnedFd, Errno> {
    let dir = rustix::fs::openat2(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
        ResolveFlags::NO_XDEV | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
    )?;
    if rustix::fs::fstat(&dir)?.st_dev != dev {
        return Err(Errno::XDEV);
    }
    Ok(dir)
}

/// Öffnet die Sperrdatei in `dir`, ohne einem Verweis zu folgen; mit
/// `create` legt es sie an. Nur eine reguläre Datei dieses Nutzers taugt.
fn open_lock(dir: BorrowedFd<'_>, create: bool) -> Result<OwnedFd, Errno> {
    let mut flags = OFlags::RDWR | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    if create {
        flags |= OFlags::CREATE;
    }
    let lock = rustix::fs::openat(dir, LOCK_FILE, flags, Mode::from_raw_mode(0o600))?;
    let stat = rustix::fs::fstat(&lock)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile || stat.st_uid != own_uid() {
        return Err(Errno::PERM);
    }
    Ok(lock)
}

/// Ob `lock` noch die Datei ist, die in `dir` unter [`LOCK_FILE`] steht.
fn lock_still_linked(dir: BorrowedFd<'_>, lock: BorrowedFd<'_>) -> bool {
    match (
        rustix::fs::statat(dir, LOCK_FILE, AtFlags::SYMLINK_NOFOLLOW),
        rustix::fs::fstat(lock),
    ) {
        (Ok(linked), Ok(held)) => same_file(&linked, &held),
        _ => false,
    }
}

/// Die Namen in `dir` ohne `.` und `..`, höchstens `room`.
fn names(dir: BorrowedFd<'_>, room: usize) -> Result<Vec<OsString>, Errno> {
    crate::worktree::dir_names(dir, room).map(|(names, _)| names)
}

/// Belegt `instance` für die Sitzung `session` und legt sie mit `create`
/// an, solange niemand die Instanz räumen kann.
///
/// Das Verzeichnis entsteht mit `0700`, die Sperrdatei darin mit `0600`.
/// Hält gerade ein anderer Daemon die exklusive Sperre, weil er eine
/// verwaiste Instanz desselben Namens räumt, wartet es höchstens
/// [`CLAIM_WAIT`] und legt danach Verzeichnis und Sperrdatei neu an.
///
/// # Errors
///
/// Ein Satz, wenn sich das Verzeichnis nicht anlegen oder belegen lässt;
/// sonst der Fehler von `create`. Scheitert `create`, gibt es die Belegung
/// wieder frei, wenn keine andere Sitzung dieses Prozesses mehr darin liegt.
pub(crate) fn enter<E>(
    instance: &Path,
    session: &OsStr,
    create: impl FnOnce() -> Result<(), E>,
    failed: impl FnOnce(String) -> E,
) -> Result<(), E> {
    // Das Warten auf einen Räumenden läuft ohne die Sperre über alle
    // Belegungen: Andere Sitzungen dieses Prozesses starten und enden
    // derweil. Eine zweite geteilte Sperre desselben Prozesses schadet nicht;
    // sie fällt weg, wenn ein anderer Faden schneller war. Gibt ein anderer
    // Faden die Instanz zwischen beiden Blicken frei, beginnt es von vorn.
    let mut claims = loop {
        let fresh = if claims().contains_key(instance) {
            None
        } else {
            match claim(instance) {
                Ok(lock) => Some(lock),
                Err(why) => return Err(failed(why)),
            }
        };
        let mut claims = claims();
        if let Some(lock) = fresh
            && !claims.contains_key(instance)
        {
            claims.insert(
                instance.to_path_buf(),
                Claim {
                    lock,
                    sessions: BTreeSet::new(),
                },
            );
        }
        if claims.contains_key(instance) {
            break claims;
        }
    };
    let created = create();
    if let Some(claim) = claims.get_mut(instance) {
        if created.is_ok() {
            claim.sessions.insert(session.to_owned());
        } else if claim.sessions.is_empty() {
            release(&mut claims, instance);
        }
    }
    created
}

/// Die geteilte Sperre auf `instance`, mit der Datei, die dort wirklich
/// steht.
fn claim(instance: &Path) -> Result<OwnedFd, String> {
    let deadline = Instant::now() + CLAIM_WAIT;
    loop {
        match try_claim(instance) {
            Ok(Some(lock)) => return Ok(lock),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            Ok(None) => {
                return Err(format!(
                    "{} is held by another process for more than {} s; another daemon is \
                     still removing what an ended one left there",
                    instance.display(),
                    CLAIM_WAIT.as_secs()
                ));
            }
            Err(err) => return Err(format!("{}: {err}", instance.display())),
        }
    }
}

/// Ein Versuch von [`claim`]: `None`, wenn jemand die Instanz gerade räumt.
fn try_claim(instance: &Path) -> Result<Option<OwnedFd>, Errno> {
    match rustix::fs::mkdirat(CWD, instance, Mode::from_raw_mode(0o700)) {
        Ok(()) | Err(Errno::EXIST) => {}
        Err(err) => return Err(err),
    }
    let dir = match open_dir(CWD, instance) {
        Ok(dir) => dir,
        // Gerade geräumt: noch einmal.
        Err(Errno::NOENT) => return Ok(None),
        Err(err) => return Err(err),
    };
    let stat = rustix::fs::fstat(&dir)?;
    if stat.st_uid != own_uid() {
        return Err(Errno::PERM);
    }
    rustix::fs::fchmod(&dir, Mode::from_raw_mode(0o700))?;
    let lock = match open_lock(dir.as_fd(), true) {
        Ok(lock) => lock,
        Err(Errno::NOENT) => return Ok(None),
        Err(err) => return Err(err),
    };
    match rustix::fs::flock(&lock, FlockOperation::NonBlockingLockShared) {
        Ok(()) => {}
        Err(Errno::WOULDBLOCK) => return Ok(None),
        Err(err) => return Err(err),
    }
    // Die Sperre gilt nur, wenn sie die Datei ist, die jetzt unter dem Pfad
    // steht: Ein Räumender kann Datei und Verzeichnis zwischen dem Öffnen
    // und der Sperre entfernt haben.
    let linked = rustix::fs::statat(CWD, instance.join(LOCK_FILE), AtFlags::SYMLINK_NOFOLLOW);
    let held = rustix::fs::fstat(&lock)?;
    Ok(linked
        .is_ok_and(|linked| same_file(&linked, &held))
        .then_some(lock))
}

/// Nach dem Entfernen des Laufzeitverzeichnisses einer Sitzung: Die Sitzung
/// fällt aus der Belegung, und war sie die letzte dieses Prozesses, wird die
/// Instanz freigegeben ([`release`]). Ein Verzeichnis außerhalb einer
/// belegten Instanz ändert nichts.
pub(crate) fn session_dir_removed(dir: &Path) {
    let (Some(instance), Some(session)) = (dir.parent(), dir.file_name()) else {
        return;
    };
    let mut claims = claims();
    let Some(claim) = claims.get_mut(instance) else {
        return;
    };
    claim.sessions.remove(session);
    if claim.sessions.is_empty() {
        release(&mut claims, instance);
    }
}

/// Gibt jede Instanz frei, die dieser Prozess noch belegt; am Ende des
/// Daemons.
pub(crate) fn release_all() {
    let mut claims = claims();
    let instances: Vec<PathBuf> = claims.keys().cloned().collect();
    for instance in instances {
        release(&mut claims, &instance);
    }
}

/// Gibt die Belegung von `instance` frei und entfernt das Verzeichnis, wenn
/// kein anderer Prozess es belegt und außer der Sperrdatei nichts darin
/// liegt.
///
/// Liegt noch etwas anderes darin, bleibt die Belegung stehen, solange dieser
/// Prozess lebt: Es kann eine Sitzung eines anderen Prozesses sein, die
/// gerade entsteht, oder ein Rest, dessen Entfernen scheiterte; den räumt
/// nach dem Ende des Prozesses der nächste Start ([`find_orphans`]).
fn release(claims: &mut BTreeMap<PathBuf, Claim>, instance: &Path) {
    let Ok(dir) = open_dir(CWD, instance) else {
        claims.remove(instance);
        return;
    };
    let only_lock = names(dir.as_fd(), 2).is_ok_and(|names| names == [OsStr::new(LOCK_FILE)]);
    if !only_lock {
        return;
    }
    let Some(claim) = claims.remove(instance) else {
        return;
    };
    // Scheitert die Wandlung, belegt ein anderer Prozess die Instanz noch;
    // er räumt sie mit seiner letzten Sitzung. Linux gibt dabei die geteilte
    // Sperre schon frei, und die Belegung endet hier ohnehin.
    if rustix::fs::flock(&claim.lock, FlockOperation::NonBlockingLockExclusive).is_err()
        || !lock_still_linked(dir.as_fd(), claim.lock.as_fd())
    {
        return;
    }
    if rustix::fs::unlinkat(&dir, LOCK_FILE, AtFlags::empty()).is_ok() {
        // Legt ein anderer Prozess gerade eine neue Sperrdatei an, scheitert
        // das mit `ENOTEMPTY`, und das Verzeichnis bleibt seins.
        let _ = rustix::fs::unlinkat(CWD, instance, AtFlags::REMOVEDIR);
    }
    drop(claim);
}

/// Eine verwaiste Instanz, exklusiv gesperrt.
#[derive(Debug)]
pub(crate) struct Orphan {
    /// Der Name unter dem Laufzeitverzeichnis.
    name: OsString,
    /// Das Verzeichnis der Instanz.
    dir: OwnedFd,
    /// Die Sperrdatei, exklusiv gesperrt, bis das Verzeichnis weg ist.
    lock: OwnedFd,
    /// Das Gerät des Verzeichnisses; das Räumen verlässt es nie.
    dev: u64,
    /// Die Einträge außer der Sperrdatei.
    pub(crate) sessions: Vec<OsString>,
}

/// Die verwaisten Instanzen unter einem Laufzeitverzeichnis.
#[derive(Debug, Default)]
pub(crate) struct Orphans {
    /// Das Laufzeitverzeichnis, für die Meldungen.
    runtime: PathBuf,
    /// Sein Deskriptor; `None` ohne Fund.
    runtime_fd: Option<OwnedFd>,
    /// Die Funde.
    pub(crate) found: Vec<Orphan>,
}

/// Die Bestandsaufnahme unter `runtime`: jede Instanz, deren Besitzer
/// nachweislich nicht mehr lebt. Die exklusiven Sperren bleiben bis
/// [`Orphans::remove`] gehalten, damit keiner eine Instanz in der Zwischenzeit
/// wieder belegt.
///
/// `own` ist der Name der Instanz des Aufrufers; nur ihr legt es eine
/// fehlende Sperrdatei an.
pub(crate) fn find_orphans(runtime: &Path, own: Option<&OsStr>) -> Orphans {
    let Ok(runtime_fd) = open_dir(CWD, runtime) else {
        return Orphans::default();
    };
    let Ok(entries) = names(runtime_fd.as_fd(), usize::MAX) else {
        return Orphans::default();
    };
    let found: Vec<Orphan> = entries
        .into_iter()
        .filter(|name| is_instance_name(name.as_bytes()))
        .filter_map(|name| {
            let create = own.is_some_and(|own| own == name);
            orphan(runtime_fd.as_fd(), name, create)
        })
        .collect();
    Orphans {
        runtime: runtime.to_path_buf(),
        runtime_fd: (!found.is_empty()).then_some(runtime_fd),
        found,
    }
}

/// `name` unter `runtime`, wenn es eine verwaiste Instanz ist.
fn orphan(runtime: BorrowedFd<'_>, name: OsString, create_lock: bool) -> Option<Orphan> {
    let runtime_dev = rustix::fs::fstat(runtime).ok()?.st_dev;
    let dir = open_dir_here(runtime, &name, runtime_dev).ok()?;
    let stat = rustix::fs::fstat(&dir).ok()?;
    if stat.st_uid != own_uid() {
        return None;
    }
    let lock = open_lock(dir.as_fd(), create_lock).ok()?;
    rustix::fs::flock(&lock, FlockOperation::NonBlockingLockExclusive).ok()?;
    #[cfg(test)]
    tests::before_compare();
    // Gesperrt ist, was jetzt unter den Namen steht, nicht was vorher dort
    // stand.
    let linked = rustix::fs::statat(runtime, &*name, AtFlags::SYMLINK_NOFOLLOW).ok()?;
    if !same_file(&linked, &stat) || !lock_still_linked(dir.as_fd(), lock.as_fd()) {
        return None;
    }
    let sessions = names(dir.as_fd(), usize::MAX)
        .ok()?
        .into_iter()
        .filter(|entry| entry != LOCK_FILE)
        .collect();
    Some(Orphan {
        name,
        dir,
        lock,
        dev: runtime_dev,
        sessions,
    })
}

/// Die Unit einer Sitzung, abgeleitet aus dem Namen ihres Verzeichnisses
/// wie beim Start (`vm-<Sitzung>`, `named` setzt den Anfang davor), nie aus
/// einer Datei darin: Ein verwaistes Verzeichnis könnte sonst jede Unit
/// des Nutzers nennen, auch die einer laufenden Sitzung. Nur ein Name aus 32
/// kleinen Hex-Ziffern ist eine Sitzung.
pub(crate) fn session_unit(
    named: impl Fn(&str) -> Result<UnitName, String>,
    session: &OsStr,
) -> Option<UnitName> {
    let session = session.to_str()?;
    let hex = session.len() == SESSION_HEX
        && session
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
    hex.then(|| named(&format!("vm-{session}")).ok()).flatten()
}

/// Warum eine Sitzung einer verwaisten Instanz liegen blieb.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kept {
    /// Sie reicht tiefer als [`MAX_DEPTH`] Ebenen.
    TooDeep,
    /// In ihr hängt ein anderes Dateisystem oder ein Bind-Mount; das wird
    /// nie betreten.
    Mount,
    /// Ein anderer Fehler.
    Io(Errno),
}

impl From<Errno> for Kept {
    fn from(err: Errno) -> Self {
        match err {
            Errno::XDEV | Errno::BUSY => Self::Mount,
            other => Self::Io(other),
        }
    }
}

/// Was beim Entfernen einer Sitzung einer verwaisten Instanz geschah.
#[derive(Debug)]
pub(crate) struct Removed {
    /// Der Pfad der Sitzung.
    pub(crate) dir: PathBuf,
    /// Ihre Unit, abgeleitet aus dem Namen ([`session_unit`]).
    pub(crate) unit: Option<UnitName>,
    /// Ob das Entfernen gelang.
    pub(crate) result: Result<(), Kept>,
}

impl Orphans {
    /// Ob nichts gefunden wurde.
    pub(crate) fn is_empty(&self) -> bool {
        self.found.is_empty()
    }

    /// Ob eine der Instanzen eine Sitzung trägt.
    pub(crate) fn has_sessions(&self) -> bool {
        self.found.iter().any(|orphan| !orphan.sessions.is_empty())
    }

    /// Stoppt je Sitzung ihre Unit (`unit_for` leitet sie aus dem Namen ab,
    /// `stop` stoppt sie), entfernt sie und danach Sperrdatei und
    /// Verzeichnis jeder Instanz, deren Sitzungen alle weg sind. Zurück kommt
    /// je Sitzung, was geschah; eine leere Instanz verschwindet ohne Eintrag.
    pub(crate) fn remove(
        self,
        unit_for: impl Fn(&OsStr) -> Option<UnitName>,
        stop: impl Fn(&UnitName),
    ) -> Vec<Removed> {
        let Some(runtime_fd) = self.runtime_fd else {
            return Vec::new();
        };
        let mut removed = Vec::new();
        for orphan in self.found {
            let base = self.runtime.join(&orphan.name);
            let mut all = true;
            for session in orphan.sessions {
                let unit = unit_for(&session);
                if let Some(unit) = &unit {
                    stop(unit);
                }
                let result = remove_tree(orphan.dir.as_fd(), &session, orphan.dev, MAX_DEPTH);
                all &= result.is_ok();
                removed.push(Removed {
                    dir: base.join(&session),
                    unit,
                    result,
                });
            }
            if all && rustix::fs::unlinkat(&orphan.dir, LOCK_FILE, AtFlags::empty()).is_ok() {
                let _ = rustix::fs::unlinkat(&runtime_fd, &*orphan.name, AtFlags::REMOVEDIR);
            }
            drop(orphan.lock);
        }
        removed
    }
}

/// Entfernt `name` unter `parent` samt Inhalt, ohne einem Verweis zu folgen
/// und ohne das Gerät `dev` zu verlassen: Ein Verweis ist ein Eintrag wie
/// jeder andere, eine Einhängung lässt den Eintrag mit [`Kept::Mount`]
/// stehen, bevor darin etwas gelöscht wird.
fn remove_tree(parent: BorrowedFd<'_>, name: &OsStr, dev: u64, depth: u32) -> Result<(), Kept> {
    match rustix::fs::unlinkat(parent, name, AtFlags::empty()) {
        Ok(()) | Err(Errno::NOENT) => return Ok(()),
        Err(Errno::ISDIR | Errno::PERM) => {}
        Err(err) => return Err(err.into()),
    }
    if depth == 0 {
        return Err(Kept::TooDeep);
    }
    let dir = open_dir_here(parent, name, dev)?;
    loop {
        let children = names(dir.as_fd(), NAMES_PER_PASS)?;
        if children.is_empty() {
            break;
        }
        for child in &children {
            remove_tree(dir.as_fd(), child, dev, depth - 1)?;
        }
    }
    match rustix::fs::unlinkat(parent, name, AtFlags::REMOVEDIR) {
        Ok(()) | Err(Errno::NOENT) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
#[path = "instance_tests.rs"]
mod tests;
