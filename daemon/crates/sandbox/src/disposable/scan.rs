//! Was die obere Schicht gegenüber dem Projekt geändert hat (HUM-350).
//!
//! Die obere Schicht eines Overlays ist kein Diff, sondern ein Baum mit
//! Sonderzeichen. Gelesen wird sie so, wie overlayfs sie schreibt, wenn die
//! Sandbox es ohne Privilegien einhängt (`userxattr`, `redirect_dir=nofollow`,
//! kein `metacopy`; gemessen mit bubblewrap 0.13.1 und Linux 7.2):
//!
//! - **Whiteout**: ein Zeichengerät `0:0` unter dem Namen einer Datei der
//!   unteren Schicht. Sie ist gelöscht.
//! - **Opakes Verzeichnis**: ein Verzeichnis mit dem erweiterten Attribut
//!   `user.overlay.opaque = "y"`. Alles, was die untere Schicht darunter hat
//!   und die obere nicht, ist gelöscht. So entsteht es, wenn der Agent ein
//!   Verzeichnis löscht und neu anlegt.
//! - **Kopie nach oben**: Eine Datei, die der Agent zum Schreiben geöffnet
//!   hat, steht ganz in der oberen Schicht, auch wenn er nichts geändert hat.
//!   Ob sie sich geändert hat, entscheidet der Inhalt, nicht ihr Dasein.
//! - Umbenannte Verzeichnisse gibt es nicht: Ohne `redirect_dir` antwortet
//!   overlayfs `EXDEV`, und Werkzeuge kopieren und löschen.
//!
//! Ein Eintrag, der etwas anderes trägt (`user.overlay.redirect`,
//! `user.overlay.metacopy`, ein Gerät, eine FIFO, ein Socket), wird
//! angezeigt, aber nie auf den Host geschrieben.

use std::ffi::OsString;
use std::fs::File;
use std::io::Read as _;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Component, Path, PathBuf};

use rustix::fs::{AtFlags, FileType, OFlags};
use rustix::io::Errno;
use sha2::{Digest as _, Sha256};

use crate::summary::{ChangeKind, display_path, path_hash};
use crate::worktree::{Entry, Kind, TreeSnapshot, dir_names, open_beneath};

use super::{MAX_DEPTH, chmod_nofollow, open_dir_forcing};

/// So viele Einträge liest der Diff höchstens aus der oberen Schicht.
pub const MAX_SCAN_ENTRIES: usize = 200_000;

/// Das erweiterte Attribut eines opaken Verzeichnisses.
const OPAQUE_XATTR: &str = "user.overlay.opaque";

/// Attribute, die eine obere Schicht ohne Privilegien nie trägt; wer sie
/// findet, schreibt nichts davon auf den Host.
const FOREIGN_XATTRS: &[&str] = &["user.overlay.redirect", "user.overlay.metacopy"];

/// So viele Einträge liest der Diff eines Klons höchstens: Er sieht das
/// ganze Projekt, und jeder Eintrag zählt zweimal, im Klon und im Projekt.
pub const MAX_CLONE_SCAN_ENTRIES: usize = 2_000_000;

/// Eine Änderung der oberen Schicht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerChange {
    /// Der Pfad relativ zum Projekt, roh. Nie angezeigt, nie über das Netz;
    /// angezeigt wird [`LayerChange::display`].
    pub rel: PathBuf,
    /// Was geschehen ist.
    pub kind: ChangeKind,
    /// Was der Eintrag danach ist; bei [`ChangeKind::Removed`] was er vorher
    /// war.
    pub what: Kind,
    /// Die Größe danach, `0` für Gelöschtes.
    pub size: u64,
    /// Die Rechte, mit denen geschrieben würde: nur `rwx` für Besitzer,
    /// Gruppe und andere.
    pub mode: u32,
    /// Die Bits, die die obere Schicht trug und die nie geschrieben werden:
    /// setuid, setgid, sticky.
    pub stripped: u32,
    /// `SHA-256` des neuen Inhalts einer Datei.
    pub sha256: Option<[u8; 32]>,
    /// Ob die Datei in der oberen Schicht mehr als einen Namen hat. Sie wird
    /// trotzdem als eigene Datei geschrieben, nie als Verweis.
    pub hardlinked: bool,
    /// Warum der Mensch zuerst hinsehen sollte: Die Datei im Projekt hat sich
    /// seit dem Start geändert. `None`: unverändert.
    pub conflict: Option<String>,
    /// Warum diese Änderung nie auf den Host geschrieben wird.
    pub refused: Option<String>,
}

impl LayerChange {
    /// Der Pfad zur Anzeige, gesäubert wie in der Zusammenfassung.
    #[must_use]
    pub fn display(&self) -> String {
        display_path(&self.rel)
    }

    /// Die Kennung, mit der ein Client diese Änderung auswählt: die ersten
    /// 16 Hex-Zeichen des `SHA-256` über die rohen Bytes des Pfades.
    #[must_use]
    pub fn path_hash(&self) -> String {
        path_hash(&self.rel)
    }

    /// Der Unterschied als Text im Format `diff -u`, wenn beide Seiten Text
    /// sind und unter [`super::DIFF_MAX_BYTES`] liegen.
    #[must_use]
    pub fn text_diff(&self, upper: BorrowedFd<'_>, lower: BorrowedFd<'_>) -> Option<String> {
        if self.what != Kind::File {
            return None;
        }
        let new = match self.kind {
            ChangeKind::Removed => Vec::new(),
            _ => read_small(upper, &self.rel)?,
        };
        let old = match self.kind {
            ChangeKind::Added => Vec::new(),
            _ => read_small(lower, &self.rel).unwrap_or_default(),
        };
        super::unified(&old, &new, &self.display())
    }
}

/// Alle Änderungen einer oberen Schicht.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeSet {
    /// Die Änderungen, sortiert nach Pfad.
    pub changes: Vec<LayerChange>,
    /// Ob ein Budget gegriffen hat oder sich etwas nicht lesen ließ; dann
    /// zeigt die Liste nicht alles, und „alles übernehmen" wird abgelehnt.
    /// Eine Liste, die so unvollständig ist, gilt nie als „nichts geändert"
    /// (HUM-357).
    pub truncated: bool,
    /// Warum die Liste unvollständig ist, höchstens `MAX_INCOMPLETE`
    /// Sätze (HUM-357).
    pub incomplete: Vec<String>,
}

/// So viele Gründe einer unvollständigen Liste stehen höchstens darin.
pub const MAX_INCOMPLETE: usize = 16;

/// Liest die obere Schicht und vergleicht sie mit dem Projekt.
///
/// `upper` und `lower` sind Deskriptoren auf die Wurzeln; jeder Zugriff
/// darunter geht über [`open_beneath`] oder `statat` mit
/// `AT_SYMLINK_NOFOLLOW` auf genau einen Namen. `baseline` ist der Stand des
/// Projekts beim Start ([`super::baseline`]); gegen ihn wird jede Änderung auf
/// einen Konflikt geprüft.
#[must_use]
pub fn scan(upper: BorrowedFd<'_>, lower: BorrowedFd<'_>, baseline: &TreeSnapshot) -> ChangeSet {
    scan_with_budget(upper, lower, baseline, MAX_SCAN_ENTRIES)
}

/// Liest einen Klon des Projekts (macOS, HUM-374) und vergleicht ihn mit dem
/// Projekt.
///
/// Ein Klon trägt keine Whiteouts: Jedes Verzeichnis zählt wie ein opakes,
/// was im Projekt liegt und im Klon fehlt, ist gelöscht. Was der Agent nicht
/// berührt hat, erkennt der Vergleich mit `baseline`: gleiche Art, Größe,
/// `mtime` und Rechte heißt unberührt, auch wenn der Mensch das Projekt dort
/// inzwischen geändert hat. Fehlt dem Klon etwas, das die Grundlage nicht
/// kennt, hat es der Mensch erst nach dem Start angelegt.
///
/// `masks` sind die Pfade, die vor dem Start aus dem Klon entfernt wurden;
/// ihr Fehlen ist keine Löschung, und etwas, das dort liegt, wird nie
/// geschrieben.
#[must_use]
pub fn scan_clone(
    clone: BorrowedFd<'_>,
    lower: BorrowedFd<'_>,
    baseline: &TreeSnapshot,
    masks: &[PathBuf],
) -> ChangeSet {
    scan_clone_with_budget(clone, lower, baseline, masks, MAX_CLONE_SCAN_ENTRIES)
}

/// Wie [`scan_clone`], mit eigenem Budget an Einträgen; für Tests.
pub(crate) fn scan_clone_with_budget(
    clone: BorrowedFd<'_>,
    lower: BorrowedFd<'_>,
    baseline: &TreeSnapshot,
    masks: &[PathBuf],
    budget: usize,
) -> ChangeSet {
    let mut scanner = Scanner {
        lower,
        baseline,
        out: ChangeSet::default(),
        seen: 0,
        budget,
        read_xattr: fgetxattr,
        clone_masks: Some(masks.iter().map(|mask| folded(mask)).collect()),
    };
    scanner.opaque_removals(clone, Path::new(""));
    scanner.walk(clone, Path::new(""), &Walk::default());
    scanner.out.changes.sort_by(|a, b| a.rel.cmp(&b.rel));
    scanner.out
}

/// Wie [`scan`], mit eigenem Budget an Einträgen; für Tests.
pub(crate) fn scan_with_budget(
    upper: BorrowedFd<'_>,
    lower: BorrowedFd<'_>,
    baseline: &TreeSnapshot,
    budget: usize,
) -> ChangeSet {
    scan_with(upper, lower, baseline, budget, fgetxattr)
}

/// Wie [`scan_with_budget`], mit eigenem Leser der Attribute; für Tests, die
/// einen Lesefehler brauchen.
pub(crate) fn scan_with(
    upper: BorrowedFd<'_>,
    lower: BorrowedFd<'_>,
    baseline: &TreeSnapshot,
    budget: usize,
    read_xattr: XattrReader,
) -> ChangeSet {
    let mut scanner = Scanner {
        lower,
        baseline,
        out: ChangeSet::default(),
        seen: 0,
        budget,
        read_xattr,
        clone_masks: None,
    };
    scanner.walk(upper, Path::new(""), &Walk::default());
    scanner.out.changes.sort_by(|a, b| a.rel.cmp(&b.rel));
    scanner.out
}

/// Ob ein Symlink mit diesem Ziel, an der Stelle `link` im Projekt, auf den
/// Host geschrieben werden darf.
///
/// Nur ein relatives, schon normalisiertes Ziel: `..` höchstens am Anfang,
/// nie hinter einem Namen, und nicht mehr davon, als `link` tief liegt.
/// Absolute Ziele und `a/../b` fallen weg. Der Grund: Ein Symlink, dessen
/// Ziel `..` hinter einem Namen trägt, wird über das Ziel dieses Namens
/// aufgelöst, und das kann selbst ein Symlink sein. `sub/d -> ..` zusammen
/// mit `e -> sub/d/..` führt lexikalisch nicht hinaus und landet doch über
/// dem Projekt. Mit der Regel hier legt jeder Symlink seine `..` von dem
/// echten Verzeichnis aus zurück, in dem er liegt, und dessen Tiefe ist
/// seine lexikalische; über die Wurzel kommt keiner, auch nicht in Ketten.
#[must_use]
pub fn target_stays_inside(link: &Path, target: &Path) -> bool {
    if target.as_os_str().is_empty() {
        return false;
    }
    let depth = link.parent().map_or(0, |parent| {
        parent
            .components()
            .filter(|component| matches!(component, Component::Normal(_)))
            .count()
    });
    let mut ups = 0_usize;
    let mut named = false;
    for component in target.components() {
        match component {
            Component::ParentDir if !named => ups += 1,
            Component::Normal(_) => named = true,
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    ups <= depth
}

/// Wo der Lauf über die obere Schicht gerade steht.
#[derive(Debug, Clone, Default)]
struct Walk {
    depth: usize,
    /// Ob ein Vorfahr opak ist: Dann ist die untere Schicht hier verdeckt,
    /// und jedes Verzeichnis verdeckt, was unten liegt und oben fehlt, auch
    /// ohne eigenes Attribut (`rm -rf build && mkdir -p build/a`).
    under_opaque: bool,
    /// Ein Symlink im Projekt, den der Agent durch ein Verzeichnis ersetzt
    /// hat; alles darunter läge im Projekt hinter diesem Symlink.
    replaced_link: Option<PathBuf>,
}

impl Walk {
    fn below(&self, opaque: bool, replaced_link: Option<PathBuf>) -> Self {
        Self {
            depth: self.depth + 1,
            under_opaque: self.under_opaque || opaque,
            replaced_link: replaced_link.or_else(|| self.replaced_link.clone()),
        }
    }

    /// Warum eine Änderung unter einem unerreichbaren Pfad nicht geschrieben
    /// wird, mit dem Fall beim Namen, wenn der Agent einen Symlink des
    /// Projekts durch ein Verzeichnis ersetzt hat.
    fn unreachable(&self, why: &str) -> String {
        match &self.replaced_link {
            Some(link) => format!(
                "{} is a symbolic link in the project that the agent replaced with a \
                 directory, and Humanitl never writes below a symbolic link; remove the link \
                 in the project and apply again",
                display_path(link)
            ),
            None => why.to_owned(),
        }
    }
}

/// Was im Projekt an einem Pfad liegt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Lower {
    /// Nichts.
    Missing,
    /// Ein Eintrag, ohne Hash.
    Present(Entry),
    /// Der Weg dorthin führt über einen Symlink oder aus dem Projekt hinaus.
    Unreachable(String),
}

/// Liest, was im Projekt an `rel` liegt, ohne einem Symlink zu folgen.
pub(crate) fn lower_at(lower: BorrowedFd<'_>, rel: &Path) -> Lower {
    let (Some(parent), Some(name)) = (rel.parent(), rel.file_name()) else {
        return Lower::Unreachable("the path has no name".to_owned());
    };
    let dir = match open_beneath(
        lower,
        parent,
        crate::worktree::DIR_ANCHOR | OFlags::DIRECTORY | OFlags::CLOEXEC,
    ) {
        Ok(dir) => dir,
        Err(Errno::NOENT | Errno::NOTDIR) => return Lower::Missing,
        Err(err) => {
            return Lower::Unreachable(format!(
                "a parent of it in the project is a symbolic link or leaves the project ({err})"
            ));
        }
    };
    match rustix::fs::statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => Lower::Present(entry_of(&dir, name, &stat)),
        Err(Errno::NOENT) => Lower::Missing,
        Err(err) => Lower::Unreachable(format!("it cannot be inspected in the project ({err})")),
    }
}

/// Ein Eintrag aus einem `stat`, wie der Schnappschuss ihn baut, ohne Hash.
pub(crate) fn entry_of(dir: impl AsFd, name: &std::ffi::OsStr, stat: &rustix::fs::Stat) -> Entry {
    let kind = match FileType::from_raw_mode(stat.st_mode) {
        FileType::RegularFile => Kind::File,
        FileType::Directory => Kind::Dir,
        FileType::Symlink => match rustix::fs::readlinkat(dir, name, Vec::new()) {
            Ok(target) => Kind::Symlink {
                target: PathBuf::from(OsString::from_vec(target.into_bytes())),
            },
            Err(_err) => Kind::Other,
        },
        _ => Kind::Other,
    };
    Entry {
        kind,
        size: u64::try_from(stat.st_size).unwrap_or(0),
        mtime_ns: i128::from(stat.st_mtime)
            .saturating_mul(1_000_000_000)
            .saturating_add(i128::from(stat.st_mtime_nsec)),
        mode: mode_bits(stat) & 0o7777,
        hash: None,
    }
}

/// Ob zwei Einträge für die Konfliktprüfung gleich sind: Art (mit dem Ziel
/// eines Symlinks), bei Dateien auch Größe, `mtime` und Rechte, bei
/// Verzeichnissen die Rechte.
pub(crate) fn same(before: &Entry, now: &Entry) -> bool {
    if before.kind != now.kind {
        return false;
    }
    match before.kind {
        Kind::File => {
            before.size == now.size && before.mtime_ns == now.mtime_ns && before.mode == now.mode
        }
        Kind::Dir => before.mode == now.mode,
        _ => true,
    }
}

/// Ob sich das Projekt an `rel` seit dem Start geändert hat; mit `subtree`
/// auch alles darunter (für das Löschen und Ersetzen eines Verzeichnisses).
pub(crate) fn conflict(
    lower: BorrowedFd<'_>,
    baseline: &TreeSnapshot,
    rel: &Path,
    subtree: bool,
) -> Option<String> {
    let now = lower_at(lower, rel);
    match (baseline.get(rel), &now) {
        (_, Lower::Unreachable(why)) => Some(why.clone()),
        (None, Lower::Missing) => baseline.truncated().then(|| {
            format!(
                "the snapshot of the project at the start was cut short after {} entries \
                 (the limit is {}, or a directory could not be read), so nobody knows whether \
                 it was there; look at the project and apply with --overwrite",
                baseline.len(),
                crate::worktree::DEFAULT_MAX_ENTRIES
            )
        }),
        (None, Lower::Present(_)) => {
            Some("it was created in the project after the start".to_owned())
        }
        (Some(_), Lower::Missing) => {
            Some("it was removed from the project after the start".to_owned())
        }
        (Some(before), Lower::Present(entry)) => {
            if !same(before, entry) {
                return Some("it changed in the project after the start".to_owned());
            }
            if subtree && entry.kind == Kind::Dir {
                return subtree_conflict(lower, baseline, rel);
            }
            None
        }
    }
}

/// Vergleicht alles unter einem Verzeichnis des Projekts mit der Grundlage.
fn subtree_conflict(lower: BorrowedFd<'_>, baseline: &TreeSnapshot, rel: &Path) -> Option<String> {
    let Ok(dir) = open_beneath(
        lower,
        rel,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
    ) else {
        return Some("its contents in the project cannot be read".to_owned());
    };
    let now = crate::worktree::snapshot_at(dir.as_fd(), &super::baseline_limits());
    if now.truncated() || baseline.truncated() {
        return Some("its contents are larger than a snapshot holds".to_owned());
    }
    let before: Vec<(&PathBuf, &Entry)> = baseline
        .entries()
        .iter()
        .filter(|(path, _)| path.starts_with(rel) && path.as_path() != rel)
        .collect();
    if before.len() != now.len() {
        return Some(
            "files below it were added or removed in the project after the start".to_owned(),
        );
    }
    for (path, entry) in before {
        let Ok(inner) = path.strip_prefix(rel) else {
            return Some("its contents cannot be compared".to_owned());
        };
        match now.get(inner) {
            Some(current) if same(entry, current) => {}
            _ => {
                return Some(format!(
                    "{} below it changed in the project after the start",
                    display_path(inner)
                ));
            }
        }
    }
    None
}

struct Scanner<'a> {
    lower: BorrowedFd<'a>,
    baseline: &'a TreeSnapshot,
    out: ChangeSet,
    seen: usize,
    /// So viele Einträge höchstens, aus oberer Schicht und verdeckter unterer
    /// zusammen.
    budget: usize,
    /// Liest ein erweitertes Attribut; `fgetxattr`, in Tests ein Fehler auf
    /// Bestellung (HUM-357, Review Runde 3).
    read_xattr: XattrReader,
    /// `Some` bei einem Klon ([`scan_clone`]), mit den entfernten Masken.
    clone_masks: Option<Vec<PathBuf>>,
}

/// Wie [`rustix::fs::fgetxattr`]: Länge des Werts in `buf` oder der Fehler.
pub(crate) type XattrReader = fn(&OwnedFd, &str, &mut [u8]) -> Result<usize, Errno>;

/// Der echte Leser.
fn fgetxattr(fd: &OwnedFd, name: &str, buf: &mut [u8]) -> Result<usize, Errno> {
    rustix::fs::fgetxattr(fd, name, buf)
}

impl Scanner<'_> {
    /// Das erste fremde Overlay-Attribut an `fd`. Lässt sich eines nicht
    /// lesen, zählt es als vorhanden (die Änderung steht abgelehnt da), und
    /// die Liste ist unvollständig (HUM-357, Review Runde 3).
    fn foreign_attr(&mut self, fd: &OwnedFd, rel: &Path) -> Option<&'static str> {
        // Ein Klon ist kein Overlay; der Agent kann dort jedes Attribut
        // setzen, und keines bedeutet etwas.
        if self.clone_masks.is_some() {
            return None;
        }
        for attr in FOREIGN_XATTRS {
            match has_xattr(self.read_xattr, fd, attr) {
                Ok(false) => {}
                Ok(true) => return Some(attr),
                Err(err) => {
                    self.incomplete(format!(
                        "the attribute {attr} of {} in the layer cannot be read ({err})",
                        shown(rel)
                    ));
                    return Some(attr);
                }
            }
        }
        None
    }

    /// Die Liste ist unvollständig, mit Grund (HUM-357): Jeder Lese- oder
    /// Öffnungsfehler in oberer oder unterer Schicht landet hier, nie als
    /// stilles Weglassen, denn eine verdeckte Löschung, die so verloren
    /// ginge, sähe wie „nichts geändert" aus.
    /// Ob `rel` eine der aus dem Klon entfernten Masken ist.
    fn is_mask(&self, rel: &Path) -> bool {
        self.clone_masks.as_ref().is_some_and(|masks| {
            let rel = folded(rel);
            masks.contains(&rel)
        })
    }

    /// Ob `rel` auf dem Weg zu einer Maske liegt. Wer dort statt eines
    /// Verzeichnisses einen Symlink hinterlässt, lenkte die Maske im Projekt
    /// auf einen anderen Ort um.
    fn above_mask(&self, rel: &Path) -> bool {
        self.clone_masks.as_ref().is_some_and(|masks| {
            let rel = folded(rel);
            masks.iter().any(|mask| mask.starts_with(&rel))
        })
    }

    /// Eine Änderung auf oder über einer Maske; sie wird nie geschrieben.
    fn push_masked(&mut self, rel: &Path, stat: &rustix::fs::Stat) {
        let mut change = Self::base(rel, ChangeKind::Added, Kind::Other, stat);
        change.refused = Some(
            "it lies on a masked path, which the sandbox profile keeps from the agent".to_owned(),
        );
        self.push(change);
    }

    /// Ob der Agent diesen Eintrag des Klons nicht berührt hat: Er steht so
    /// in der Grundlage. Über einem Overlay nie, dort hat er ihn angefasst.
    fn untouched(
        &self,
        up: BorrowedFd<'_>,
        name: &std::ffi::OsStr,
        rel: &Path,
        stat: &rustix::fs::Stat,
    ) -> bool {
        self.clone_masks.is_some()
            && self
                .baseline
                .get(rel)
                .is_some_and(|before| same(before, &entry_of(up, name, stat)))
    }

    fn incomplete(&mut self, why: String) {
        self.out.truncated = true;
        if self.out.incomplete.len() < MAX_INCOMPLETE {
            self.out.incomplete.push(why);
        }
    }

    fn walk(&mut self, up: BorrowedFd<'_>, rel: &Path, at: &Walk) {
        let room = self.budget.saturating_sub(self.seen);
        let (names, more) = match dir_names(up, room) {
            Ok(read) => read,
            Err(err) => {
                self.incomplete(format!(
                    "{} in the layer cannot be read ({err})",
                    shown(rel)
                ));
                return;
            }
        };
        if more {
            self.incomplete(format!("the list stops at {} entries", self.budget));
        }
        self.seen += names.len();
        for name in &names {
            let child = rel.join(name);
            let stat = match rustix::fs::statat(up, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => stat,
                Err(err) => {
                    self.incomplete(format!(
                        "{} in the layer cannot be inspected ({err})",
                        shown(&child)
                    ));
                    continue;
                }
            };
            if self.is_mask(&child) {
                self.push_masked(&child, &stat);
                continue;
            }
            let lower = lower_at(self.lower, &child);
            let untouched = self.untouched(up, name.as_os_str(), &child, &stat);
            let kind = FileType::from_raw_mode(stat.st_mode);
            if untouched && kind != FileType::Directory {
                continue;
            }
            if kind != FileType::Directory && self.above_mask(&child) {
                self.push_masked(&child, &stat);
                continue;
            }
            match kind {
                FileType::CharacterDevice if stat.st_rdev == 0 && self.clone_masks.is_none() => {
                    self.whiteout(&child, &lower);
                }
                FileType::Directory if untouched => {
                    self.directory(up, name, &child, None, &lower, at);
                }
                FileType::Directory => self.directory(up, name, &child, Some(&stat), &lower, at),
                FileType::RegularFile => self.file(up, name, &child, &stat, &lower, at),
                FileType::Symlink => self.symlink(up, name, &child, &lower, at),
                other => {
                    let mut change = Self::base(&child, ChangeKind::Added, Kind::Other, &stat);
                    change.refused = Some(format!(
                        "it is a {}, and Humanitl writes only files, directories and \
                         symbolic links to the host",
                        type_name(other)
                    ));
                    self.push(change);
                }
            }
        }
    }

    fn base(rel: &Path, kind: ChangeKind, what: Kind, stat: &rustix::fs::Stat) -> LayerChange {
        LayerChange {
            rel: rel.to_path_buf(),
            kind,
            what,
            size: u64::try_from(stat.st_size).unwrap_or(0),
            mode: mode_bits(stat) & 0o777,
            stripped: mode_bits(stat) & 0o7000,
            sha256: None,
            hardlinked: false,
            conflict: None,
            refused: None,
        }
    }

    fn push(&mut self, mut change: LayerChange) {
        if change.conflict.is_none() {
            let subtree = change.kind == ChangeKind::Removed || change.kind == ChangeKind::Modified;
            change.conflict = conflict(self.lower, self.baseline, &change.rel, subtree);
        }
        self.out.changes.push(change);
    }

    fn whiteout(&mut self, rel: &Path, lower: &Lower) {
        let (what, refused) = match lower {
            Lower::Present(entry) => (entry.kind.clone(), None),
            // Nichts darunter: Der Agent hat etwas gelöscht, das es im
            // Projekt nicht gibt. Nichts zu tun.
            Lower::Missing => return,
            // Gelöscht würde über einen Symlink im Projekt; das geschieht nie.
            Lower::Unreachable(why) => (Kind::Other, Some(why.clone())),
        };
        self.push(LayerChange {
            rel: rel.to_path_buf(),
            kind: ChangeKind::Removed,
            what,
            size: 0,
            mode: 0,
            stripped: 0,
            sha256: None,
            hardlinked: false,
            conflict: None,
            refused,
        });
    }

    /// Ein Verzeichnis der Schicht. Ohne `stat` hat der Agent es selbst nicht
    /// berührt (nur bei einem Klon); was darunter liegt, zählt trotzdem.
    fn directory(
        &mut self,
        up: BorrowedFd<'_>,
        name: &std::ffi::OsStr,
        rel: &Path,
        stat: Option<&rustix::fs::Stat>,
        lower: &Lower,
        at: &Walk,
    ) {
        let dir = match open_dir_forcing(up, name) {
            Ok(dir) => dir,
            Err(err) => {
                self.incomplete(format!(
                    "{} in the layer cannot be opened ({err})",
                    shown(rel)
                ));
                return;
            }
        };
        let foreign = self.foreign_attr(&dir, rel);
        let lower_is_dir = matches!(lower, Lower::Present(entry) if entry.kind == Kind::Dir);
        if let Some(stat) = stat {
            let change = match lower {
                Lower::Present(entry) if entry.kind == Kind::Dir => (entry.mode & 0o777
                    != mode_bits(stat) & 0o777)
                    .then(|| Self::base(rel, ChangeKind::ModeChanged, Kind::Dir, stat)),
                Lower::Present(_) => Some(Self::base(rel, ChangeKind::Modified, Kind::Dir, stat)),
                Lower::Missing => Some(Self::base(rel, ChangeKind::Added, Kind::Dir, stat)),
                Lower::Unreachable(why) => {
                    let mut change = Self::base(rel, ChangeKind::Added, Kind::Dir, stat);
                    change.refused = Some(at.unreachable(why));
                    Some(change)
                }
            };
            if let Some(mut change) = change {
                if let Some(attr) = foreign {
                    change.refused = Some(format!("it carries the overlay attribute {attr}"));
                }
                self.push(change);
            } else if let Some(attr) = foreign {
                let mut change = Self::base(rel, ChangeKind::Modified, Kind::Dir, stat);
                change.refused = Some(format!("it carries the overlay attribute {attr}"));
                self.push(change);
            }
        }
        let read_opaque = if self.clone_masks.is_some() {
            Ok(true)
        } else {
            is_opaque(self.read_xattr, &dir)
        };
        let opaque = match read_opaque {
            Ok(opaque) => opaque,
            // Unbekannt heißt weder opak noch nicht opak: Die Liste ist
            // unvollständig und wartet. Als opak gezählt, stünden Löschungen
            // darin, die es nicht gab.
            Err(err) => {
                self.incomplete(format!(
                    "whether {} in the layer hides the project's entries cannot be read ({err})",
                    shown(rel)
                ));
                false
            }
        };
        if lower_is_dir && (opaque || at.under_opaque) {
            self.opaque_removals(dir.as_fd(), rel);
        }
        if at.depth + 1 >= MAX_DEPTH {
            self.incomplete(format!(
                "{} lies deeper than {MAX_DEPTH} levels",
                shown(rel)
            ));
            return;
        }
        let replaced =
            matches!(lower, Lower::Present(entry) if matches!(entry.kind, Kind::Symlink { .. }))
                .then(|| rel.to_path_buf());
        self.walk(dir.as_fd(), rel, &at.below(opaque, replaced));
    }

    /// Ein opakes Verzeichnis verdeckt alles darunter in der unteren
    /// Schicht: Was dort liegt und oben fehlt, ist gelöscht.
    fn opaque_removals(&mut self, up_dir: BorrowedFd<'_>, rel: &Path) {
        // Unlesbar heißt nicht leer (HUM-357, Review): Ein Projektordner mit
        // Rechten `030` verdeckt sonst jede Löschung darunter, und die Liste
        // sähe aus, als hätte der Agent nichts geändert.
        let lower_dir = match open_beneath(
            self.lower,
            rel,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        ) {
            Ok(dir) => dir,
            Err(err) => {
                self.incomplete(format!(
                    "{} in the project cannot be read, so what the agent deleted below it \
                     is not known ({err})",
                    shown(rel)
                ));
                return;
            }
        };
        let room = self.budget.saturating_sub(self.seen);
        let (names, more) = match dir_names(lower_dir.as_fd(), room) {
            Ok(read) => read,
            Err(err) => {
                self.incomplete(format!(
                    "{} in the project cannot be listed, so what the agent deleted below it \
                     is not known ({err})",
                    shown(rel)
                ));
                return;
            }
        };
        if more {
            self.incomplete(format!("the list stops at {} entries", self.budget));
        }
        // Die verdeckten Einträge zählen gegen dasselbe Budget wie die der
        // oberen Schicht (Review Runde 2).
        self.seen += names.len();
        for name in names {
            let above = rustix::fs::statat(up_dir, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW);
            let child = rel.join(&name);
            if self.is_mask(&child) {
                continue;
            }
            match above {
                // Bei einem Klon heißt „fehlt" nur dann gelöscht, wenn es
                // beim Start da war; neuere Dateien hat der Mensch angelegt.
                // Ist die Grundlage abgeschnitten, entscheidet der Konflikt.
                Err(Errno::NOENT)
                    if self.clone_masks.is_some()
                        && self.baseline.get(&child).is_none()
                        && !self.baseline.truncated() => {}
                Err(Errno::NOENT) => {
                    let lower = lower_at(self.lower, &child);
                    self.whiteout(&child, &lower);
                }
                Ok(_) => {}
                Err(err) => self.incomplete(format!(
                    "{} in the layer cannot be inspected ({err})",
                    shown(&child)
                )),
            }
        }
    }

    fn file(
        &mut self,
        up: BorrowedFd<'_>,
        name: &std::ffi::OsStr,
        rel: &Path,
        stat: &rustix::fs::Stat,
        lower: &Lower,
        at: &Walk,
    ) {
        let fd = match open_upper_file(up, name) {
            Ok(fd) => fd,
            Err(err) => {
                self.incomplete(format!(
                    "{} in the layer cannot be opened ({err})",
                    shown(rel)
                ));
                return;
            }
        };
        let foreign = self.foreign_attr(&fd, rel);
        let Some(new_hash) = hash_fd(fd) else {
            self.incomplete(format!("{} in the layer cannot be read", shown(rel)));
            return;
        };
        let mut change = match lower {
            Lower::Missing => Self::base(rel, ChangeKind::Added, Kind::File, stat),
            Lower::Present(entry) if entry.kind == Kind::File => {
                let same_content = entry.size == u64::try_from(stat.st_size).unwrap_or(0)
                    && lower_hash(self.lower, rel) == Some(new_hash);
                let same_mode = entry.mode & 0o777 == mode_bits(stat) & 0o777;
                match (same_content, same_mode) {
                    // Nur nach oben kopiert, nicht geändert.
                    (true, true) if foreign.is_none() => return,
                    (true, false) => Self::base(rel, ChangeKind::ModeChanged, Kind::File, stat),
                    // Mit fremdem Attribut zeigt sich auch eine unveränderte
                    // Kopie, damit sie abgelehnt dasteht.
                    (true, true) | (false, _) => {
                        Self::base(rel, ChangeKind::Modified, Kind::File, stat)
                    }
                }
            }
            Lower::Present(_) => Self::base(rel, ChangeKind::Modified, Kind::File, stat),
            Lower::Unreachable(why) => {
                let mut change = Self::base(rel, ChangeKind::Added, Kind::File, stat);
                change.refused = Some(at.unreachable(why));
                change
            }
        };
        if let Some(attr) = foreign {
            change.refused = Some(format!("it carries the overlay attribute {attr}"));
        }
        change.sha256 = Some(new_hash);
        change.hardlinked = stat.st_nlink > 1;
        self.push(change);
    }

    fn symlink(
        &mut self,
        up: BorrowedFd<'_>,
        name: &std::ffi::OsStr,
        rel: &Path,
        lower: &Lower,
        at: &Walk,
    ) {
        let target = match rustix::fs::readlinkat(up, name, Vec::new()) {
            Ok(target) => target,
            Err(err) => {
                self.incomplete(format!(
                    "the link {} in the layer cannot be read ({err})",
                    shown(rel)
                ));
                return;
            }
        };
        let target = PathBuf::from(OsString::from_vec(target.into_bytes()));
        let what = Kind::Symlink {
            target: target.clone(),
        };
        let kind = match lower {
            Lower::Present(entry) if entry.kind == what => return,
            Lower::Present(_) => ChangeKind::Modified,
            Lower::Missing | Lower::Unreachable(_) => ChangeKind::SymlinkAdded,
        };
        let mut change = LayerChange {
            rel: rel.to_path_buf(),
            kind,
            what,
            size: u64::try_from(target.as_os_str().len()).unwrap_or(0),
            mode: 0o777,
            stripped: 0,
            sha256: None,
            hardlinked: false,
            conflict: None,
            refused: None,
        };
        if !target_stays_inside(rel, &target) {
            change.refused = Some(format!(
                "it is a symbolic link to {}, which leaves the project or is not a plain \
                 relative path inside it (`..` only at the start)",
                display_path(&target)
            ));
        } else if let Lower::Unreachable(why) = lower {
            change.refused = Some(at.unreachable(why));
        }
        self.push(change);
    }
}

/// Ein Pfad ohne Unterschied der ASCII-Schreibweise: APFS ist meist
/// unempfindlich dafür, und `.ENV` im Klon wäre im Projekt `.env`.
fn folded(path: &Path) -> PathBuf {
    PathBuf::from(OsString::from_vec(
        path.as_os_str().as_bytes().to_ascii_lowercase(),
    ))
}

/// Die Rechte und die Art aus einem `stat`; `RawMode` ist auf Linux `u32`, auf
/// macOS `u16`.
#[allow(clippy::useless_conversion)]
pub(crate) fn mode_bits(stat: &rustix::fs::Stat) -> u32 {
    u32::from(stat.st_mode)
}

/// Ein Pfad der Schicht für einen Satz, gesäubert wie in der Liste; die
/// Wurzel heißt „the project root".
fn shown(rel: &Path) -> String {
    if rel.as_os_str().is_empty() {
        "the project root".to_owned()
    } else {
        display_path(rel)
    }
}

fn type_name(kind: FileType) -> &'static str {
    match kind {
        FileType::Fifo => "FIFO",
        FileType::Socket => "socket",
        FileType::CharacterDevice => "character device",
        FileType::BlockDevice => "block device",
        _ => "special file",
    }
}

/// Ob `name` an `fd` steht. Nur `ENODATA` heißt „fehlt", und `ENOTSUP`: Ein
/// Dateisystem ohne erweiterte Attribute kann keines tragen, auch keines,
/// das Overlay geschrieben hätte. Jeder andere Fehler ist ein Fehler
/// (HUM-357, Review Runde 3).
fn has_xattr(read: XattrReader, fd: &OwnedFd, name: &str) -> Result<bool, Errno> {
    let mut buf = [0_u8; 8];
    match read(fd, name, &mut buf[..]) {
        Ok(_) | Err(Errno::RANGE) => Ok(true),
        Err(Errno::NODATA | Errno::NOTSUP) => Ok(false),
        Err(err) => Err(err),
    }
}

/// Ob `fd` ein opakes Verzeichnis ist (`user.overlay.opaque = "y"`); Fehler
/// wie in [`has_xattr`]. Ein längerer Wert (`ERANGE`) ist nicht `"y"`.
fn is_opaque(read: XattrReader, fd: &OwnedFd) -> Result<bool, Errno> {
    let mut buf = [0_u8; 8];
    match read(fd, OPAQUE_XATTR, &mut buf[..]) {
        Ok(len) => Ok(buf.get(..len) == Some(b"y".as_slice())),
        Err(Errno::NODATA | Errno::NOTSUP | Errno::RANGE) => Ok(false),
        Err(err) => Err(err),
    }
}

/// Öffnet eine Datei der oberen Schicht; fehlt das Leserecht (der Agent darf
/// `chmod 000`), bekommt sie es. Die obere Schicht gehört dem Daemon, und
/// nach dem Ende schreibt niemand mehr hinein.
pub(crate) fn open_upper_file(
    dir: BorrowedFd<'_>,
    name: &std::ffi::OsStr,
) -> Result<OwnedFd, Errno> {
    match open_file(dir, Path::new(name)) {
        Err(Errno::ACCESS) => {
            let stat = rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)?;
            if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
                return Err(Errno::INVAL);
            }
            chmod_nofollow(
                dir,
                name,
                (mode_bits(&stat) & 0o7777) | 0o400,
                FileType::RegularFile,
            )?;
            open_file(dir, Path::new(name))
        }
        other => other,
    }
}

fn lower_hash(lower: BorrowedFd<'_>, rel: &Path) -> Option<[u8; 32]> {
    let fd = open_file(lower, rel).ok()?;
    hash_fd(fd)
}

fn hash_fd(fd: OwnedFd) -> Option<[u8; 32]> {
    let mut file = File::from(fd);
    let mut hasher = Sha256::new();
    let mut buf = vec![0_u8; 64 * 1024];
    loop {
        match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(buf.get(..n)?),
            Err(_) => return None,
        }
    }
    Some(hasher.finalize().into())
}

/// Öffnet eine gewöhnliche Datei unter `root` zum Lesen; alles andere ist
/// ein Fehler. Fehlt das Leserecht (der Agent darf `chmod 000`), bekommt die
/// Datei es nicht: gelesen wird dann nicht, und die Änderung ist
/// abgeschnitten.
pub(crate) fn open_file(root: BorrowedFd<'_>, rel: &Path) -> Result<OwnedFd, Errno> {
    let fd = open_beneath(
        root,
        rel,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
    )?;
    let stat = rustix::fs::fstat(&fd)?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(Errno::INVAL);
    }
    Ok(fd)
}

fn read_small(root: BorrowedFd<'_>, rel: &Path) -> Option<Vec<u8>> {
    let fd = open_file(root, rel).ok()?;
    let limit = super::DIFF_MAX_BYTES;
    let bytes = crate::worktree::read_capped(fd, limit + 1).ok()?;
    (u64::try_from(bytes.len()).ok()? <= limit).then_some(bytes)
}
