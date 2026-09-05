//! Der Blick des Hosts in das Projektverzeichnis: Schnappschuss, Diff,
//! Symlink-Erkennung und das sichere Öffnen (HUM-043).
//!
//! `/work` ist der erste der beiden offenen Kanäle (`docs/SECURITY.md`
//! Abschnitt 3.2, BACKLOG.md 4.2). Er wird nicht geschlossen, sondern
//! deklariert und beobachtet: Vor dem Start nimmt der Daemon einen
//! Schnappschuss des Baums, nach dem Ende des Sandbox-Laufs einen zweiten, und
//! der Unterschied der beiden ist das, was ein Mensch danach zu sehen bekommt.
//!
//! # Warum jeder Zugriff hier `openat2` benutzt
//!
//! Der zweite Schnappschuss läuft host-seitig unter dem Konto des Daemons über
//! denselben Baum, in den der Agent gerade noch geschrieben hat. Jede
//! Pfadauflösung darin ist ein Angriffsziel:
//!
//! - Ein Symlink `x -> /etc` verwandelt `x/passwd` in `/etc/passwd`. Ein
//!   naives `read_dir` oder `File::open` über einen zusammengesetzten Pfad
//!   läse den Host und schriebe den Fund in eine Zusammenfassung, die ein
//!   Mensch liest.
//! - Ein Verzeichnis, das zwischen `statat` und `openat` zu einem Symlink
//!   wird, hebelt jede Prüfung aus, die vor dem Öffnen stattfindet (TOCTOU).
//! - Ein Pfad mit `..` führt aus `/work` hinaus, ohne dass ein einziger
//!   Symlink beteiligt wäre.
//!
//! Deshalb wird jeder Verzeichnis- und Dateizugriff relativ zu einem
//! Deskriptor auf die Wurzel geöffnet, mit
//! `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS`: Der Kernel
//! selbst weigert sich, die Auflösung über die Wurzel hinaus oder über einen
//! Symlink zu führen, und zwar in demselben Aufruf, der die Datei öffnet. Es
//! gibt kein Fenster zwischen Prüfung und Benutzung, weil es keine getrennte
//! Prüfung gibt. `Path::canonicalize` kommt hier nicht vor; ein aufgelöster
//! Pfad wäre schon wieder eine Prüfung, die vor der Benutzung liegt.
//!
//! Metadaten liest [`rustix::fs::statat`] mit `AT_SYMLINK_NOFOLLOW`, und der
//! Name, den es bekommt, ist immer genau ein Verzeichniseintrag ohne `/`.
//! Damit gibt es auch dort keine Auflösung, die über die Wurzel hinausführen
//! könnte: Der letzte Bestandteil wird nicht verfolgt, und einen vorletzten
//! gibt es nicht.
//!
//! Für Kernel vor 5.6 (`openat2` antwortet `ENOSYS`) bleibt die Garantie
//! dieselbe, nur langsamer: [`open_beneath`] geht dann Bestandteil für
//! Bestandteil mit `openat` und `O_NOFOLLOW`, lehnt `..` und absolute Pfade
//! lexikalisch ab und meldet den Wechsel einmal als `SANDBOX_021` (Info).
//!
//! # Budgets
//!
//! Ein Baum kann größer sein, als eine Zusammenfassung tragen kann, und ein
//! Agent kann ihn absichtlich größer machen. Der Lauf endet deshalb nie in
//! einem hängenden Daemon, sondern im Flag [`TreeSnapshot::truncated`]:
//! höchstens [`SnapshotLimits::max_entries`] Einträge, höchstens [`MAX_DEPTH`]
//! Ebenen, gehasht wird nur bis [`SnapshotLimits::hash_max_bytes`].
//! Symlink-Schleifen gibt es nicht, weil kein Symlink verfolgt wird.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::Read as _;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};

use humanitl_core::diagnostics::codes::{SANDBOX_011, SANDBOX_021};
use humanitl_core::http::sha256;
use humanitl_core::{Diagnostic, Severity};
use rustix::fs::{AtFlags, FileType, Mode, OFlags, RawDir, ResolveFlags};
use rustix::io::Errno;

/// So viele Einträge nimmt ein Schnappschuss höchstens auf.
pub const DEFAULT_MAX_ENTRIES: usize = 200_000;

/// Bis zu dieser Größe wird eine Datei gehasht; darüber entscheiden Größe und
/// `mtime` über „geändert".
pub const DEFAULT_HASH_MAX_BYTES: u64 = 4 * 1024 * 1024;

/// So tief steigt der Lauf höchstens; darunter gilt der Baum als abgeschnitten.
pub const MAX_DEPTH: usize = 64;

/// Verzeichnisnamen, die auf jeder Ebene übersprungen werden.
pub const DEFAULT_SKIP_NAMES: &[&str] =
    &["node_modules", "target", ".venv", "__pycache__", ".cache"];

/// Pfade relativ zur Wurzel, die übersprungen werden.
///
/// Zwei Listen statt einer: Ein bloßer Name kann `.git/objects` nicht
/// ausdrücken, ohne jedes `objects/` im Baum mitzunehmen.
pub const DEFAULT_SKIP_PATHS: &[&str] = &[".git/objects"];

/// So groß ist der Puffer, mit dem ein Verzeichnis gelesen wird.
///
/// Ein Eintrag ist höchstens 255 Bytes Name plus Kopf; mit 64 KiB liest
/// `getdents64` jedes übliche Verzeichnis in wenigen Runden, und der Puffer ist
/// nie zu klein für einen einzelnen Eintrag (was `EINVAL` gäbe).
const DIR_BUFFER_BYTES: usize = 64 * 1024;

/// Die Flags, mit denen jeder Zugriff unter der Wurzel aufgelöst wird.
///
/// Öffentlich, damit ein Test sie nennen kann: Fällt eines der drei weg, ist
/// das eine Änderung an einer Sicherheitsaussage und keine an einer
/// Konstanten.
pub const RESOLVE_FLAGS: ResolveFlags = ResolveFlags::BENEATH
    .union(ResolveFlags::NO_SYMLINKS)
    .union(ResolveFlags::NO_MAGICLINKS);

/// Der Zustand der Kernel-Prüfung: 0 unbekannt, 1 `openat2`, 2 Fallback.
static RESOLVER: AtomicU8 = AtomicU8::new(0);

/// Wie ein Pfad unter der Wurzel aufgelöst wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// `openat2` mit `RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS`, ein Aufruf, in
    /// dem Prüfung und Öffnen dasselbe sind (Linux 5.6 und neuer).
    Openat2,
    /// `openat` mit `O_NOFOLLOW` je Bestandteil, dazu die lexikalische
    /// Ablehnung von `..` und absoluten Pfaden.
    PerComponent,
}

impl Resolution {
    /// Der Name für Protokoll und Befund.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Openat2 => "openat2",
            Self::PerComponent => "openat_per_component",
        }
    }
}

/// Wie dieser Kernel Pfade unter der Wurzel auflöst.
///
/// Beim ersten Aufruf entscheidet der Kernel selbst: ein `openat2` auf das
/// Arbeitsverzeichnis mit `O_PATH`. Antwortet er `ENOSYS`, bleibt es für die
/// Laufzeit des Prozesses beim Fallback.
#[must_use]
pub fn resolution() -> Resolution {
    match RESOLVER.load(Ordering::Relaxed) {
        1 => Resolution::Openat2,
        2 => Resolution::PerComponent,
        _ => {
            let probe = rustix::fs::openat2(
                rustix::fs::CWD,
                ".",
                OFlags::PATH | OFlags::CLOEXEC,
                Mode::empty(),
                ResolveFlags::empty(),
            );
            let found = match probe {
                Err(Errno::NOSYS | Errno::OPNOTSUPP) => Resolution::PerComponent,
                _ => Resolution::Openat2,
            };
            RESOLVER.store(
                match found {
                    Resolution::Openat2 => 1,
                    Resolution::PerComponent => 2,
                },
                Ordering::Relaxed,
            );
            found
        }
    }
}

/// Der Befund, wenn dieser Kernel `openat2` nicht kennt: `SANDBOX_021` (Info).
///
/// `None`, solange `openat2` da ist. Der Aufrufer meldet ihn einmal je Lauf;
/// die Garantie gilt in beiden Fällen, der Fallback braucht nur mehr Aufrufe.
#[must_use]
pub fn resolution_diagnostic() -> Option<Diagnostic> {
    if resolution() == Resolution::Openat2 {
        return None;
    }
    Some(
        Diagnostic::builder(SANDBOX_021, Severity::Info)
            .why(
                "this kernel does not have openat2(2) (Linux 5.6 and newer), so the project \
                 directory is walked with openat and O_NOFOLLOW per component; symlinks are still \
                 never followed and nothing outside the project is opened, the walk only needs \
                 more system calls"
                    .to_owned(),
            )
            .build(),
    )
}

/// Die Grenzen eines Schnappschusses.
#[derive(Debug, Clone)]
pub struct SnapshotLimits {
    /// So viele Einträge höchstens; danach ist der Schnappschuss abgeschnitten.
    pub max_entries: usize,
    /// Bis zu dieser Größe wird gehasht.
    pub hash_max_bytes: u64,
    /// Verzeichnisnamen, die auf jeder Ebene übersprungen werden.
    pub skip_names: Vec<&'static str>,
    /// Pfade relativ zur Wurzel, die übersprungen werden.
    pub skip_paths: Vec<&'static str>,
}

impl Default for SnapshotLimits {
    fn default() -> Self {
        Self {
            max_entries: DEFAULT_MAX_ENTRIES,
            hash_max_bytes: DEFAULT_HASH_MAX_BYTES,
            skip_names: DEFAULT_SKIP_NAMES.to_vec(),
            skip_paths: DEFAULT_SKIP_PATHS.to_vec(),
        }
    }
}

/// Was ein Eintrag im Baum ist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Kind {
    /// Eine gewöhnliche Datei.
    File,
    /// Ein Verzeichnis.
    Dir,
    /// Ein symbolischer Verweis, mit seinem ungelesenen Ziel.
    Symlink {
        /// Der Inhalt des Verweises, wörtlich und nie aufgelöst.
        target: PathBuf,
    },
    /// Alles andere: Socket, FIFO, Gerät.
    Other,
}

impl Kind {
    /// Der Name für Anzeige und Zusammenfassung.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Dir => "dir",
            Self::Symlink { .. } => "symlink",
            Self::Other => "other",
        }
    }
}

/// Ein Eintrag des Schnappschusses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Was der Eintrag ist.
    pub kind: Kind,
    /// Größe in Bytes.
    pub size: u64,
    /// Änderungszeit in Nanosekunden seit der Epoche.
    pub mtime_ns: i128,
    /// Die Rechte-Bits (`st_mode & 0o7777`). Ohne sie wäre
    /// [`FileChange::ModeChanged`] nicht berechenbar.
    pub mode: u32,
    /// `SHA-256` des Inhalts, wenn die Datei klein genug war.
    ///
    /// Der Hash entscheidet über „geändert", nicht die `mtime`: Agenten
    /// benutzen `touch`, und manche Werkzeuge erhalten die `mtime` beim
    /// Schreiben.
    pub hash: Option<[u8; 32]>,
}

/// Der Baum unter `/work` zu einem Zeitpunkt.
///
/// Die Pfade sind relativ zur Wurzel, ohne führendes `/`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeSnapshot {
    entries: BTreeMap<PathBuf, Entry>,
    truncated: bool,
}

impl TreeSnapshot {
    /// Die Einträge, sortiert nach Pfad.
    #[must_use]
    pub const fn entries(&self) -> &BTreeMap<PathBuf, Entry> {
        &self.entries
    }

    /// Ob ein Budget gegriffen hat und der Baum unvollständig ist.
    #[must_use]
    pub const fn truncated(&self) -> bool {
        self.truncated
    }

    /// Wie viele Einträge erfasst wurden.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Ob nichts erfasst wurde.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Der Eintrag zu einem Pfad relativ zur Wurzel.
    #[must_use]
    pub fn get(&self, path: &Path) -> Option<&Entry> {
        self.entries.get(path)
    }
}

/// Ein Unterschied zwischen zwei Schnappschüssen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileChange {
    /// Neu hinzugekommen.
    Added(PathBuf),
    /// Inhalt geändert.
    Modified(PathBuf),
    /// Verschwunden.
    Removed(PathBuf),
    /// Ein neuer symbolischer Verweis.
    SymlinkAdded {
        /// Der Verweis, relativ zur Wurzel.
        path: PathBuf,
        /// Das ungelesene Ziel.
        target: PathBuf,
        /// Ob das Ziel aus `/work` hinausführt.
        escapes: bool,
    },
    /// Nur die Rechte-Bits haben sich geändert.
    ModeChanged(PathBuf),
}

impl FileChange {
    /// Der Pfad, den die Änderung betrifft.
    #[must_use]
    pub fn path(&self) -> &Path {
        match self {
            Self::Added(path)
            | Self::Modified(path)
            | Self::Removed(path)
            | Self::ModeChanged(path)
            | Self::SymlinkAdded { path, .. } => path,
        }
    }

    /// Der Name der Art in `snake_case`, wie ihn Zusammenfassung und
    /// Oberfläche führen.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Added(_) => "added",
            Self::Modified(_) => "modified",
            Self::Removed(_) => "removed",
            Self::SymlinkAdded { .. } => "symlink_added",
            Self::ModeChanged(_) => "mode_changed",
        }
    }
}

/// Öffnet die Wurzel des Projektverzeichnisses.
///
/// Nur dieser eine Pfad wird als Ganzes aufgelöst, und zwar mit den Rechten
/// des Daemons: Er kommt aus der Konfiguration des Nutzers
/// (`sandbox.work_dir`), nicht aus dem Baum, und darf deshalb ein Symlink
/// sein. Alles unterhalb wird nur noch relativ zu dem Deskriptor geöffnet, den
/// diese Funktion liefert.
///
/// # Errors
///
/// `SANDBOX_011`, wenn das Verzeichnis fehlt oder keines ist.
pub fn open_root(root: &Path) -> Result<OwnedFd, Diagnostic> {
    rustix::fs::open(
        root,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|err| {
        Diagnostic::builder(SANDBOX_011, Severity::Error)
            .why(format!(
                "the project directory {} could not be opened for the snapshot ({err})",
                root.display()
            ))
            .build()
    })
}

/// Öffnet einen Pfad unterhalb der Wurzel, ohne je einen Symlink zu verfolgen.
///
/// `rel` ist relativ zum Deskriptor `root` und muss unter ihm bleiben. Das
/// prüft nicht diese Funktion, sondern der Kernel: `RESOLVE_BENEATH` bricht
/// jede Auflösung ab, die die Wurzel verlässt (`EXDEV`), `RESOLVE_NO_SYMLINKS`
/// jede, die über einen Symlink führt (`ELOOP`), und `RESOLVE_NO_MAGICLINKS`
/// die über `/proc/self/fd/…`.
///
/// # Errors
///
/// Der `errno` des Kernels. Für den Aufrufer bedeutsam sind `EXDEV` (der Pfad
/// führt aus der Wurzel hinaus), `ELOOP` (ein Bestandteil ist ein Symlink) und
/// `ENOENT`.
pub fn open_beneath(root: BorrowedFd<'_>, rel: &Path, oflags: OFlags) -> Result<OwnedFd, Errno> {
    open_beneath_with(root, rel, oflags, resolution())
}

/// Wie [`open_beneath`], mit ausdrücklich gewähltem Weg.
///
/// Der Grund, warum es diese Funktion gibt: [`resolution`] entscheidet einmal
/// je Prozess, und auf jedem Kernel ab 5.6 fiele [`Resolution::PerComponent`]
/// nie an. Der Fallback muss dieselbe Garantie halten wie `openat2`, und eine
/// Garantie, die kein Test je ausführt, ist keine. Die Tests fahren denselben
/// Baum durch beide Wege.
///
/// # Errors
///
/// Wie [`open_beneath`].
pub fn open_beneath_with(
    root: BorrowedFd<'_>,
    rel: &Path,
    oflags: OFlags,
    how: Resolution,
) -> Result<OwnedFd, Errno> {
    if !is_beneath(rel) {
        return Err(Errno::XDEV);
    }
    // Der leere Pfad ist das Verzeichnis selbst; ohne `AT_EMPTY_PATH` wäre er
    // `ENOENT`. `Path::parent` liefert ihn für einen Namen ohne Verzeichnis.
    let rel = if rel.as_os_str().is_empty() {
        Path::new(".")
    } else {
        rel
    };
    match how {
        Resolution::Openat2 => rustix::fs::openat2(root, rel, oflags, Mode::empty(), RESOLVE_FLAGS),
        Resolution::PerComponent => open_per_component(root, rel, oflags),
    }
}

/// Der Fallback für Kernel ohne `openat2`.
///
/// Bestandteil für Bestandteil, jeder Zwischenschritt mit
/// `O_DIRECTORY | O_NOFOLLOW`: Ein Symlink in der Mitte scheitert damit mit
/// `ELOOP`, ohne dass ein Pfad zusammengesetzt oder aufgelöst würde. `..` und
/// absolute Pfade hat [`is_beneath`] schon vorher ausgeschlossen.
fn open_per_component(root: BorrowedFd<'_>, rel: &Path, oflags: OFlags) -> Result<OwnedFd, Errno> {
    let names: Vec<&OsStr> = rel
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    let Some((last, parents)) = names.split_last() else {
        return rustix::fs::openat(root, ".", oflags, Mode::empty());
    };

    let mut current: Option<OwnedFd> = None;
    for name in parents {
        let dir = rustix::fs::openat(
            current.as_ref().map_or(root, AsFd::as_fd),
            *name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?;
        current = Some(dir);
    }
    rustix::fs::openat(
        current.as_ref().map_or(root, AsFd::as_fd),
        *last,
        oflags | OFlags::NOFOLLOW,
        Mode::empty(),
    )
}

/// Ob ein relativer Pfad rein lexikalisch unter der Wurzel bleibt.
///
/// Absolut, mit `..` oder mit einem Wurzel-Bestandteil: nein. Die Prüfung
/// ersetzt `RESOLVE_BENEATH` nicht, sie ergänzt es — und im Fallback ist sie
/// die einzige, die `..` abfängt.
#[must_use]
pub fn is_beneath(rel: &Path) -> bool {
    !rel.is_absolute()
        && rel
            .components()
            .all(|component| matches!(component, Component::Normal(_) | Component::CurDir))
}

/// Liest eine Datei unterhalb der Wurzel, höchstens `max_bytes` davon.
///
/// # Errors
///
/// `SANDBOX_011`, wenn der Pfad nicht unter der Wurzel liegt, ein Bestandteil
/// ein Symlink ist oder die Datei nicht lesbar ist.
pub fn read_beneath(
    root: BorrowedFd<'_>,
    rel: &Path,
    max_bytes: u64,
) -> Result<Vec<u8>, Diagnostic> {
    let fd = open_beneath(
        root,
        rel,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
    )
    .map_err(|err| read_failed(rel, err))?;
    read_capped(fd, max_bytes).map_err(|err| read_failed(rel, err))
}

fn read_failed(rel: &Path, err: impl core::fmt::Display) -> Diagnostic {
    Diagnostic::builder(SANDBOX_011, Severity::Warning)
        .why(format!(
            "{} could not be read below the project directory ({err}); it stays out of the \
             session summary",
            rel.display()
        ))
        .build()
}

/// Liest höchstens `max_bytes` aus einem geöffneten Deskriptor.
fn read_capped(fd: OwnedFd, max_bytes: u64) -> Result<Vec<u8>, std::io::Error> {
    let file = File::from(fd);
    let cap = usize::try_from(max_bytes).unwrap_or(usize::MAX);
    let mut out = Vec::new();
    // Ein Byte mehr als erlaubt: Wächst die Datei zwischen `statat` und `read`,
    // fällt das auf, statt unbemerkt abgeschnitten zu werden. Der Puffer bleibt
    // trotzdem begrenzt.
    let mut limited = file.take(max_bytes.saturating_add(1));
    limited.read_to_end(&mut out)?;
    out.truncate(cap);
    Ok(out)
}

/// Ob das Ziel eines Symlinks aus der Wurzel hinausführt.
///
/// Rein lexikalisch, das Ziel wird nie aufgelöst: Ein absolutes Ziel führt
/// immer hinaus, ein relatives genau dann, wenn seine `..` tiefer greifen, als
/// der Verweis selbst liegt. `link` ist der Pfad des Verweises relativ zur
/// Wurzel.
///
/// Das Ziel eines Verweises kann sich ändern, während der Baum gelesen wird,
/// und ein aufgelöstes Ziel wäre eine Aussage über einen Zeitpunkt, der schon
/// vorbei ist. Die lexikalische Antwort ist die einzige, die stabil bleibt.
#[must_use]
pub fn escapes(link: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        return true;
    }
    let mut depth = link.parent().map_or(0, |parent| {
        parent
            .components()
            .filter(|component| matches!(component, Component::Normal(_)))
            .count()
    });
    for component in target.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if depth == 0 {
                    return true;
                }
                depth -= 1;
            }
            Component::Normal(_) => depth += 1,
            Component::RootDir | Component::Prefix(_) => return true,
        }
    }
    false
}

/// Nimmt einen Schnappschuss des Baums unter `root`.
///
/// # Errors
///
/// `SANDBOX_011`, wenn die Wurzel nicht zu öffnen ist. Ein einzelner Eintrag,
/// der sich nicht lesen lässt, ist kein Fehler: Er fehlt, und der
/// Schnappschuss ist [`TreeSnapshot::truncated`].
pub fn snapshot(root: &Path, limits: &SnapshotLimits) -> Result<TreeSnapshot, Diagnostic> {
    let fd = open_root(root)?;
    Ok(snapshot_at(fd.as_fd(), limits))
}

/// Wie [`snapshot`], aber auf einem schon geöffneten Wurzel-Deskriptor.
#[must_use]
pub fn snapshot_at(root: BorrowedFd<'_>, limits: &SnapshotLimits) -> TreeSnapshot {
    let mut walker = Walker {
        limits,
        out: TreeSnapshot::default(),
    };
    walker.walk(root, Path::new(""), 0);
    walker.out
}

/// Der Lauf über den Baum: hält die Grenzen und das Ergebnis.
struct Walker<'a> {
    limits: &'a SnapshotLimits,
    out: TreeSnapshot,
}

impl Walker<'_> {
    fn walk(&mut self, dir: BorrowedFd<'_>, rel: &Path, depth: usize) {
        let Ok(names) = dir_names(dir) else {
            self.out.truncated = true;
            return;
        };
        for name in names {
            if self.out.entries.len() >= self.limits.max_entries {
                self.out.truncated = true;
                return;
            }
            let child = rel.join(&name);
            if self.skipped(&child, &name) {
                continue;
            }
            let Ok(stat) = rustix::fs::statat(dir, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW)
            else {
                self.out.truncated = true;
                continue;
            };
            let raw_mode = stat.st_mode;
            // `RawMode` ist auf Linux `u32`; diese Crate baut nur dort.
            let mode = raw_mode & 0o7777;
            let size = u64::try_from(stat.st_size).unwrap_or(0);
            let mtime_ns = i128::from(stat.st_mtime)
                .saturating_mul(1_000_000_000)
                .saturating_add(i128::from(stat.st_mtime_nsec));
            let entry = |kind: Kind, hash: Option<[u8; 32]>| Entry {
                kind,
                size,
                mtime_ns,
                mode,
                hash,
            };

            match FileType::from_raw_mode(raw_mode) {
                FileType::Directory => {
                    self.out
                        .entries
                        .insert(child.clone(), entry(Kind::Dir, None));
                    self.descend(dir, &name, &child, depth);
                }
                FileType::Symlink => {
                    match rustix::fs::readlinkat(dir, name.as_os_str(), Vec::new()) {
                        Ok(target) => {
                            let target = PathBuf::from(OsString::from_vec(target.into_bytes()));
                            self.out
                                .entries
                                .insert(child, entry(Kind::Symlink { target }, None));
                        }
                        Err(_err) => self.out.truncated = true,
                    }
                }
                FileType::RegularFile => {
                    let hash = if size <= self.limits.hash_max_bytes {
                        self.hash_file(dir, &name, size)
                    } else {
                        None
                    };
                    self.out.entries.insert(child, entry(Kind::File, hash));
                }
                _ => {
                    self.out.entries.insert(child, entry(Kind::Other, None));
                }
            }
        }
    }

    /// Steigt in ein Unterverzeichnis, solange das Tiefenbudget reicht.
    fn descend(&mut self, dir: BorrowedFd<'_>, name: &OsStr, child: &Path, depth: usize) {
        if depth + 1 >= MAX_DEPTH {
            self.out.truncated = true;
            return;
        }
        match open_beneath(
            dir,
            Path::new(name),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        ) {
            Ok(sub) => self.walk(sub.as_fd(), child, depth + 1),
            Err(_err) => self.out.truncated = true,
        }
    }

    fn hash_file(&mut self, dir: BorrowedFd<'_>, name: &OsStr, size: u64) -> Option<[u8; 32]> {
        let opened = open_beneath(
            dir,
            Path::new(name),
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        );
        let Ok(fd) = opened else {
            self.out.truncated = true;
            return None;
        };
        match read_capped(fd, size.max(1)) {
            Ok(bytes) => Some(sha256(&bytes)),
            Err(_err) => {
                self.out.truncated = true;
                None
            }
        }
    }

    fn skipped(&self, child: &Path, name: &OsStr) -> bool {
        if self
            .limits
            .skip_names
            .iter()
            .any(|skip| name.as_bytes() == skip.as_bytes())
        {
            return true;
        }
        self.limits
            .skip_paths
            .iter()
            .any(|skip| child == Path::new(skip))
    }
}

/// Die Namen der Einträge eines Verzeichnisses, ohne `.` und `..`.
fn dir_names(dir: BorrowedFd<'_>) -> Result<Vec<OsString>, Errno> {
    let mut buffer = Vec::with_capacity(DIR_BUFFER_BYTES);
    let mut iter = RawDir::new(dir, buffer.spare_capacity_mut());
    let mut out = Vec::new();
    while let Some(entry) = iter.next() {
        let entry = entry?;
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        out.push(OsString::from_vec(name.to_vec()));
    }
    Ok(out)
}

/// Der Unterschied zweier Schnappschüsse, sortiert nach Pfad.
///
/// Eine Datei, die ihren Inhalt **und** ihre Rechte geändert hat, erscheint
/// zweimal: einmal als [`FileChange::Modified`], einmal als
/// [`FileChange::ModeChanged`]. Beides ist eine eigene Aussage, und ein
/// `chmod +x` auf eine sonst unveränderte Datei ist genau die Änderung, die
/// eine Zusammenfassung zeigen muss.
#[must_use]
pub fn diff(before: &TreeSnapshot, after: &TreeSnapshot) -> Vec<FileChange> {
    let mut out = Vec::new();
    for (path, entry) in &after.entries {
        match before.entries.get(path) {
            None => out.push(added(path, entry)),
            Some(old) => {
                if old.kind != entry.kind {
                    out.push(added(path, entry));
                } else if changed(old, entry) {
                    out.push(FileChange::Modified(path.clone()));
                }
                if old.mode != entry.mode {
                    out.push(FileChange::ModeChanged(path.clone()));
                }
            }
        }
    }
    for path in before.entries.keys() {
        if !after.entries.contains_key(path) {
            out.push(FileChange::Removed(path.clone()));
        }
    }
    out.sort_by(|a, b| {
        a.path()
            .cmp(b.path())
            .then_with(|| a.as_str().cmp(b.as_str()))
    });
    out
}

/// Der Eintrag als „neu": ein Symlink mit seinem Ziel, alles andere schlicht.
fn added(path: &Path, entry: &Entry) -> FileChange {
    match &entry.kind {
        Kind::Symlink { target } => FileChange::SymlinkAdded {
            path: path.to_path_buf(),
            target: target.clone(),
            escapes: escapes(path, target),
        },
        _ => FileChange::Added(path.to_path_buf()),
    }
}

/// Ob sich der Inhalt zweier gleichartiger Einträge unterscheidet.
///
/// Für Dateien entscheidet der Hash, sobald beide einen haben; ohne Hash
/// (Datei über [`SnapshotLimits::hash_max_bytes`]) entscheiden Größe und
/// `mtime`. Verzeichnisse ändern sich nie im Sinne dieses Diffs — was in ihnen
/// liegt, steht als eigener Eintrag da.
fn changed(old: &Entry, new: &Entry) -> bool {
    match (&old.kind, &new.kind) {
        (Kind::Dir, Kind::Dir) => false,
        (Kind::Symlink { target: a }, Kind::Symlink { target: b }) => a != b,
        _ => match (old.hash, new.hash) {
            (Some(a), Some(b)) => a != b,
            _ => old.size != new.size || old.mtime_ns != new.mtime_ns,
        },
    }
}
