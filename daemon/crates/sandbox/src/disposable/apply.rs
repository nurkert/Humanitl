//! Änderungen der oberen Schicht in das Projekt übernehmen (HUM-350).
//!
//! Dies ist der neue Schreibweg zurück auf den Host, und er schreibt nur,
//! was ein Mensch ausgewählt hat. Die Regeln:
//!
//! 1. **Erst prüfen, dann schreiben.** Jede ausgewählte Änderung wird vorher
//!    geprüft; scheitert eine, wird nichts geschrieben ([`apply`]).
//! 2. **Nie über einen Symlink.** Das Elternverzeichnis jeder Änderung wird
//!    mit `openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS)` geöffnet; ein
//!    Symlink irgendwo im Pfad bricht ab (`ELOOP`). Fehlende Verzeichnisse
//!    entstehen Schritt für Schritt mit `mkdirat` und `O_NOFOLLOW`.
//! 3. **Nur Dateien, Verzeichnisse und Symlinks, die im Projekt bleiben.**
//!    Ein Symlink, dessen Ziel das Projekt verlässt, ein Gerät, eine FIFO
//!    oder ein Socket wird nie geschrieben.
//! 4. **Keine Sonderbits.** Geschrieben werden nur `rwx` für Besitzer,
//!    Gruppe und andere; setuid, setgid und sticky fallen weg. Eine Datei mit
//!    mehreren Namen in der oberen Schicht wird je Name als eigene Datei
//!    geschrieben, nie als harter Verweis.
//! 5. **Nichts überschreiben, was der Mensch geändert hat**, außer er sagt es
//!    ([`ApplyOptions::overwrite`]): Verglichen wird mit dem Stand beim Start
//!    (Art, Größe, `mtime`, Rechte).
//! 6. **Atomar je Datei.** Eine Datei entsteht unter einem eigenen Namen im
//!    Zielverzeichnis (`O_CREAT | O_EXCL | O_NOFOLLOW`) und wird erst danach
//!    mit `renameat` an ihren Platz gesetzt; `renameat` ersetzt einen Symlink
//!    am Ziel, statt ihm zu folgen.

use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::path::{Component, Path, PathBuf};

use humanitl_core::diagnostics::codes::SANDBOX_031;
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use rustix::io::Errno;

use crate::summary::{ChangeKind, display_path};
use crate::worktree::{Kind, TreeSnapshot, open_beneath};

use super::scan::{
    ChangeSet, LayerChange, Lower, conflict, entry_of, lower_at, open_upper_file, same,
    target_stays_inside,
};

/// So viele Pfade nennt ein Befund höchstens beim Namen.
const NAMED_IN_FINDING: usize = 5;

/// Wie übernommen wird.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyOptions {
    /// Auch Dateien schreiben, die der Mensch seit dem Start im Projekt
    /// geändert hat. Ohne diesen Schalter ist jeder Konflikt ein Abbruch vor
    /// dem ersten Schreiben.
    pub overwrite: bool,
}

/// Was übernommen wurde.
#[derive(Debug, Default)]
pub struct ApplyReport {
    /// Die geschriebenen Änderungen, in der Reihenfolge des Schreibens.
    pub applied: Vec<PathBuf>,
    /// Der Pfad und der Befund, an dem das Schreiben abbrach; alles davor
    /// steht in [`ApplyReport::applied`].
    pub failed: Option<(PathBuf, Diagnostic)>,
}

/// Wählt Änderungen nach ihrer Kennung ([`LayerChange::path_hash`]); eine
/// leere Liste heißt alle.
///
/// # Errors
///
/// `SANDBOX_031` für eine Kennung, die keine Änderung trägt oder die zwei
/// tragen, und für „alle", wenn die Liste abgeschnitten ist: Was sie nicht
/// zeigt, hat niemand gesehen.
pub fn select<'a>(set: &'a ChangeSet, ids: &[String]) -> Result<Vec<&'a LayerChange>, Diagnostic> {
    if ids.is_empty() {
        if set.truncated {
            return Err(refused(
                "the list of changes is cut short (too many entries or too deep), so \
                 \"apply all\" would write changes no one has seen"
                    .to_owned(),
                FixAction::CopyCommand("humanitl sandbox discard".to_owned()),
            ));
        }
        return Ok(set.changes.iter().collect());
    }
    let mut by_id: BTreeMap<String, Vec<&LayerChange>> = BTreeMap::new();
    for change in &set.changes {
        by_id.entry(change.path_hash()).or_default().push(change);
    }
    let mut out = Vec::new();
    for id in ids {
        match by_id.get(id.trim()).map(Vec::as_slice) {
            Some([one]) => out.push(*one),
            Some(_) => {
                return Err(refused(
                    format!("the id {id} names more than one change"),
                    FixAction::CopyCommand("humanitl sandbox changes".to_owned()),
                ));
            }
            None => {
                return Err(refused(
                    format!("no pending change has the id {id}"),
                    FixAction::CopyCommand("humanitl sandbox changes".to_owned()),
                ));
            }
        }
    }
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out.dedup_by(|a, b| a.rel == b.rel);
    Ok(out)
}

/// Übernimmt die gewählten Änderungen aus `upper` nach `lower`.
///
/// Die Konflikte werden hier noch einmal gegen `baseline` geprüft, nicht aus
/// der Liste übernommen: Zwischen Anzeige und Übernahme kann der Mensch
/// weitergearbeitet haben.
///
/// # Errors
///
/// `SANDBOX_031` vor dem ersten Schreiben, wenn eine gewählte Änderung
/// abgelehnt ist oder einen Konflikt hat (ohne
/// [`ApplyOptions::overwrite`]). Ein Fehler beim Schreiben selbst ist kein
/// `Err`, sondern [`ApplyReport::failed`], damit der Aufrufer weiß, was schon
/// geschrieben ist.
pub fn apply(
    upper: BorrowedFd<'_>,
    lower: BorrowedFd<'_>,
    baseline: &TreeSnapshot,
    chosen: &[&LayerChange],
    options: ApplyOptions,
) -> Result<ApplyReport, Diagnostic> {
    let mut problems: Vec<String> = Vec::new();
    let mut planned: Vec<(&LayerChange, Expected)> = Vec::with_capacity(chosen.len());
    for change in chosen {
        if let Some(why) = &change.refused {
            problems.push(format!("{} is never written: {why}", change.display()));
            continue;
        }
        if let Kind::Symlink { target } = &change.what
            && !target_stays_inside(&change.rel, target)
        {
            problems.push(format!(
                "{} is never written: its target leaves the project",
                change.display()
            ));
            continue;
        }
        if !options.overwrite {
            let subtree = matches!(change.kind, ChangeKind::Removed | ChangeKind::Modified);
            if let Some(why) = conflict(lower, baseline, &change.rel, subtree) {
                problems.push(format!("{}: {why}", change.display()));
                continue;
            }
        }
        // Was jetzt dort liegt; unmittelbar vor dem Schreiben muss es noch so
        // sein, sonst hat der Mensch währenddessen geändert.
        match Expected::observe(lower, change) {
            Ok(expected) => planned.push((change, expected)),
            Err(why) => problems.push(format!("{}: {why}", change.display())),
        }
    }
    if !problems.is_empty() {
        let count = problems.len();
        let named: Vec<String> = problems.into_iter().take(NAMED_IN_FINDING).collect();
        let more = count.saturating_sub(NAMED_IN_FINDING);
        let tail = if more > 0 {
            format!(" (and {more} more)")
        } else {
            String::new()
        };
        return Err(refused(
            format!(
                "nothing was written, because {count} of the selected changes cannot be \
                 applied as they are: {}{tail}",
                named.join("; ")
            ),
            FixAction::CopyCommand("humanitl sandbox apply --overwrite".to_owned()),
        ));
    }

    let mut report = ApplyReport::default();
    let mut modes = DirModes::default();
    planned.sort_by(|a, b| a.0.rel.cmp(&b.0.rel));
    for (change, expected) in planned {
        match apply_one(upper, lower, change, &expected, &mut modes) {
            Ok(()) => report.applied.push(change.rel.clone()),
            Err(why) => {
                report.failed = Some((change.rel.clone(), partial(change, &why, &report)));
                break;
            }
        }
    }
    if let Err((rel, why)) = modes.finish(lower)
        && report.failed.is_none()
    {
        let diagnostic = refused(
            format!(
                "the mode of the directory {} could not be set ({why}); every change was \
                 written",
                display_path(&rel)
            ),
            FixAction::CopyCommand("humanitl sandbox changes".to_owned()),
        );
        report.failed = Some((rel, diagnostic));
    }
    Ok(report)
}

/// Der Befund, wenn das Schreiben mittendrin abbrach.
fn partial(change: &LayerChange, why: &str, report: &ApplyReport) -> Diagnostic {
    let leftovers = if matches!(change.kind, ChangeKind::Removed | ChangeKind::Modified) {
        "; a removal that stopped midway can leave directories with mode 0700 and \
         directories named .humanitl-flat-* in the project, remove them by hand"
    } else {
        ""
    };
    refused(
        format!(
            "writing {} to the project failed ({why}); {} change(s) before it were written, \
             the rest waits in the upper layer{leftovers}",
            change.display(),
            report.applied.len()
        ),
        FixAction::CopyCommand("humanitl sandbox changes".to_owned()),
    )
}

/// Was die Vorprüfung im Projekt an der Stelle einer Änderung sah.
#[derive(Debug)]
pub(crate) struct Expected {
    /// Der Eintrag selbst.
    entry: Lower,
    /// Alles darunter, wenn dort ein Verzeichnis liegt, das die Änderung
    /// löscht oder ersetzt: Jeder Eintrag wird unmittelbar vor seinem
    /// Löschen damit verglichen.
    tree: Option<TreeSnapshot>,
}

impl Expected {
    /// Liest den Stand an `change.rel` und, wenn ein Verzeichnis dort
    /// gelöscht oder ersetzt würde, den Baum darunter.
    ///
    /// # Errors
    ///
    /// Der Grund, wenn ein Verzeichnis gelöscht oder ersetzt würde, dessen
    /// Baum sich nicht vollständig lesen lässt: Ohne ihn gäbe es nichts, womit
    /// jeder Eintrag vor dem Löschen verglichen würde, und das Löschen müsste
    /// sich Zugang erzwingen (Review Runde 3). Dann wird nichts geschrieben.
    pub(crate) fn observe(lower: BorrowedFd<'_>, change: &LayerChange) -> Result<Self, String> {
        let entry = lower_at(lower, &change.rel);
        let replaces_dir = matches!(&entry, Lower::Present(found) if found.kind == Kind::Dir)
            && (change.kind == ChangeKind::Removed || change.what != Kind::Dir);
        if !replaces_dir {
            return Ok(Self { entry, tree: None });
        }
        let dir = open_beneath(
            lower,
            &change.rel,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        )
        .map_err(|err| {
            format!(
                "the directory in the project cannot be read ({err}), so what lies below it \
                 cannot be checked before it is removed; nothing is removed unchecked"
            )
        })?;
        let tree = crate::worktree::snapshot_at(dir.as_fd(), &super::baseline_limits());
        if tree.truncated() {
            return Err(
                "what lies below the directory in the project cannot be read completely (an \
                 unreadable directory, or more entries than a snapshot holds), so it cannot be \
                 checked before it is removed; nothing is removed unchecked"
                    .to_owned(),
            );
        }
        Ok(Self {
            entry,
            tree: Some(tree),
        })
    }
}

/// Die Rechte der Verzeichnisse, die erst am Ende gesetzt werden.
///
/// Ein Verzeichnis entsteht mit `rwx` für den Besitzer, damit hineingeschrieben
/// werden kann, auch wenn der Agent es mit `chmod 000` hinterließ; seine
/// endgültigen Rechte bekommt es zuletzt, die tiefsten zuerst, damit der Weg zu
/// jedem noch offen ist. Vorher wird jedes noch einmal geprüft: derselbe Inode
/// mit denselben Rechten wie beim Vormerken, sonst hat der Mensch dort
/// inzwischen etwas geändert, und es bleibt, wie es ist.
#[derive(Debug, Default)]
pub(crate) struct DirModes(Vec<DeferredMode>);

#[derive(Debug)]
struct DeferredMode {
    rel: PathBuf,
    mode: u32,
    dev: u64,
    ino: u64,
    seen_mode: u32,
}

impl DirModes {
    /// Merkt das Verzeichnis `rel` mit seinen endgültigen Rechten vor.
    pub(crate) fn later(
        &mut self,
        lower: BorrowedFd<'_>,
        rel: &Path,
        mode: u32,
    ) -> Result<(), String> {
        let dir = open_beneath(
            lower,
            rel,
            OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        )
        .map_err(|err| parent_problem(rel, err))?;
        let stat =
            rustix::fs::fstat(&dir).map_err(|err| format!("it cannot be inspected ({err})"))?;
        self.0.push(DeferredMode {
            rel: rel.to_path_buf(),
            mode: mode & 0o777,
            dev: stat.st_dev,
            ino: stat.st_ino,
            seen_mode: stat.st_mode & 0o7777,
        });
        Ok(())
    }

    pub(crate) fn finish(mut self, lower: BorrowedFd<'_>) -> Result<(), (PathBuf, String)> {
        self.0
            .sort_by_key(|deferred| std::cmp::Reverse(deferred.rel.components().count()));
        for deferred in self.0 {
            let rel = deferred.rel;
            let dir = open_beneath(
                lower,
                &rel,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            )
            .map_err(|err| (rel.clone(), err.to_string()))?;
            let stat = rustix::fs::fstat(&dir).map_err(|err| (rel.clone(), err.to_string()))?;
            if stat.st_dev != deferred.dev
                || stat.st_ino != deferred.ino
                || stat.st_mode & 0o7777 != deferred.seen_mode
            {
                return Err((
                    rel,
                    "it changed in the project while the changes were being applied, so its \
                     mode was left as it is"
                        .to_owned(),
                ));
            }
            // Über den geprüften Deskriptor, nicht über den Pfad.
            rustix::fs::chmod(
                format!("/proc/self/fd/{}", std::os::fd::AsRawFd::as_raw_fd(&dir)),
                Mode::from_raw_mode(deferred.mode),
            )
            .map_err(|err| (rel.clone(), err.to_string()))?;
        }
        Ok(())
    }
}

fn refused(why: String, fix: FixAction) -> Diagnostic {
    Diagnostic::builder(SANDBOX_031, Severity::Error)
        .why(why)
        .fix(fix)
        .build()
}

/// Ob im Projekt an `rel` noch liegt, was vor dem ersten Schreiben dort lag.
///
/// Gerufen unmittelbar vor dem Schritt, der das Projekt an dieser Stelle
/// ändert (`renameat`, `unlinkat`, `mkdirat`). Zwischen dieser Prüfung und dem
/// Schritt bleibt ein Fenster von wenigen Systemaufrufen; Humanitl sperrt den
/// Ordner des Menschen nicht (`docs/SECURITY.md` 5.2).
fn unchanged(lower: BorrowedFd<'_>, rel: &Path, expected: &Expected) -> Result<(), String> {
    let now = lower_at(lower, rel);
    let still = match (&expected.entry, &now) {
        (Lower::Missing, Lower::Missing) => true,
        (Lower::Present(before), Lower::Present(entry)) => same(before, entry),
        _ => false,
    };
    if still {
        Ok(())
    } else {
        Err(
            "it changed in the project while the changes were being applied, so it was \
             left as it is"
                .to_owned(),
        )
    }
}

/// Schreibt eine Änderung.
pub(crate) fn apply_one(
    upper: BorrowedFd<'_>,
    lower: BorrowedFd<'_>,
    change: &LayerChange,
    expected: &Expected,
    modes: &mut DirModes,
) -> Result<(), String> {
    let (Some(parent_rel), Some(name)) = (change.rel.parent(), change.rel.file_name()) else {
        return Err("the path has no name".to_owned());
    };
    // A deletion never creates the directories it would delete from: when the
    // parent is gone, so is what lay in it.
    if change.kind == ChangeKind::Removed {
        return match open_beneath(
            lower,
            parent_rel,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        ) {
            Ok(parent) => {
                unchanged(lower, &change.rel, expected)?;
                remove(&parent, name, expected)
            }
            Err(Errno::NOENT | Errno::NOTDIR) => Ok(()),
            Err(err) => Err(parent_problem(parent_rel, err)),
        };
    }
    let parent = ensure_dir(upper, lower, parent_rel, modes)?;
    let present = lower_at(lower, &change.rel);
    if let Lower::Unreachable(why) = &present {
        return Err(why.clone());
    }
    let lower_dir = matches!(&present, Lower::Present(entry) if entry.kind == Kind::Dir);
    match &change.what {
        Kind::Dir => {
            if lower_dir {
                // Nur die Rechte; gesetzt am Ende.
                unchanged(lower, &change.rel, expected)?;
            } else {
                unchanged(lower, &change.rel, expected)?;
                if matches!(present, Lower::Present(_)) {
                    remove(&parent, name, expected)?;
                }
                match rustix::fs::mkdirat(
                    &parent,
                    name,
                    Mode::from_raw_mode((change.mode & 0o777) | 0o700),
                ) {
                    Ok(()) | Err(Errno::EXIST) => {}
                    Err(err) => return Err(format!("the directory cannot be created ({err})")),
                }
            }
            modes.later(lower, &change.rel, change.mode)
        }
        // Inhalt und reine Rechte gleich: ein neuer Inode an der Stelle, nie
        // `fchmod` auf den alten. Ein harter Verweis darauf außerhalb des
        // Projekts bliebe sonst nicht unberührt.
        Kind::File => {
            if lower_dir {
                unchanged(lower, &change.rel, expected)?;
                remove(&parent, name, expected)?;
                write_file(upper, &change.rel, &parent, name, change.mode & 0o777, None)
            } else {
                let check = || unchanged(lower, &change.rel, expected);
                write_file(
                    upper,
                    &change.rel,
                    &parent,
                    name,
                    change.mode & 0o777,
                    Some(&check),
                )
            }
        }
        Kind::Symlink { target } => {
            if !target_stays_inside(&change.rel, target) {
                return Err("its target leaves the project".to_owned());
            }
            unchanged(lower, &change.rel, expected)?;
            if lower_dir {
                remove(&parent, name, expected)?;
            }
            let temp = temp_name();
            rustix::fs::symlinkat(target.as_os_str(), &parent, &temp)
                .map_err(|err| format!("the symbolic link cannot be created ({err})"))?;
            rustix::fs::renameat(&parent, &temp, &parent, name).map_err(|err| {
                let _ = rustix::fs::unlinkat(&parent, &temp, AtFlags::empty());
                format!("the symbolic link cannot be put in place ({err})")
            })
        }
        Kind::Other => Err("it is not a file, directory or symbolic link".to_owned()),
    }
}

/// Löscht `name` unter `parent`. Ein Verzeichnis wird Eintrag für Eintrag
/// gelöscht, jeder unmittelbar vorher mit dem verglichen, was die
/// Vorprüfung sah; was sich geändert hat oder neu dazukam, hält das Löschen
/// an (`partial`).
///
/// Ohne Baum (dort lag bei der Vorprüfung kein Verzeichnis) nur ein
/// einzelnes `unlinkat` ohne `AT_REMOVEDIR`: Hat der Mensch die Datei
/// inzwischen durch ein Verzeichnis ersetzt, scheitert es (`EISDIR`), und die
/// Übernahme hält mit `partial` an, statt ein ungeprüftes Verzeichnis zu
/// löschen. Rekursiv gelöscht wird im Projekt nur über `remove_checked`.
pub(crate) fn remove(parent: &OwnedFd, name: &OsStr, expected: &Expected) -> Result<(), String> {
    match &expected.tree {
        Some(tree) => remove_checked(parent.as_fd(), name, None, tree, 0),
        None => match rustix::fs::unlinkat(parent, name, AtFlags::empty()) {
            Ok(()) | Err(Errno::NOENT) => Ok(()),
            Err(err) => Err(format!(
                "it cannot be removed ({err}); if it became a directory in the project \
                 meanwhile, it is left as it is"
            )),
        },
    }
}

/// Löscht einen Teilbaum gegen `tree`; `rel` ist der Pfad im Teilbaum,
/// `None` für dessen Wurzel, die schon geprüft ist.
fn remove_checked(
    parent: BorrowedFd<'_>,
    name: &OsStr,
    rel: Option<&Path>,
    tree: &TreeSnapshot,
    depth: usize,
) -> Result<(), String> {
    let changed = |rel: &Path| {
        format!(
            "{} below it changed in the project while the changes were being applied, so the \
             removal stopped there",
            display_path(rel)
        )
    };
    let stat = rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|err| format!("it cannot be inspected ({err})"))?;
    if let Some(rel) = rel {
        let now = entry_of(parent, name, &stat);
        match tree.get(rel) {
            Some(before) if same(before, &now) => {}
            _ => return Err(changed(rel)),
        }
    }
    if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
        if depth >= super::MAX_DEPTH {
            return Err("it is deeper than a removal checks".to_owned());
        }
        let dir = super::open_dir_forcing(parent, name)
            .map_err(|err| format!("it cannot be opened ({err})"))?;
        let (names, _) = crate::worktree::dir_names(dir.as_fd(), usize::MAX)
            .map_err(|err| format!("it cannot be read ({err})"))?;
        for child in names {
            let child_rel = rel.map_or_else(|| PathBuf::from(&child), |rel| rel.join(&child));
            remove_checked(dir.as_fd(), &child, Some(&child_rel), tree, depth + 1)?;
        }
        rustix::fs::unlinkat(parent, name, AtFlags::REMOVEDIR)
            .map_err(|err| format!("it cannot be removed ({err})"))
    } else {
        rustix::fs::unlinkat(parent, name, AtFlags::empty())
            .map_err(|err| format!("it cannot be removed ({err})"))
    }
}

/// Schreibt den Inhalt der Datei `rel` der oberen Schicht als `name` in
/// `parent`, über einen eigenen Namen und `renameat`.
fn write_file(
    upper: BorrowedFd<'_>,
    rel: &Path,
    parent: &OwnedFd,
    name: &OsStr,
    mode: u32,
    check: Option<&dyn Fn() -> Result<(), String>>,
) -> Result<(), String> {
    let (Some(upper_parent), Some(upper_name)) = (rel.parent(), rel.file_name()) else {
        return Err("the path has no name".to_owned());
    };
    let upper_dir = open_beneath(
        upper,
        upper_parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
    )
    .map_err(|err| format!("it cannot be found in the upper layer ({err})"))?;
    let source = open_upper_file(upper_dir.as_fd(), upper_name)
        .map_err(|err| format!("it cannot be read from the upper layer ({err})"))?;
    let temp = temp_name();
    let target = rustix::fs::openat(
        parent,
        &temp,
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|err| format!("a temporary file cannot be created next to it ({err})"))?;
    let written = (|| -> std::io::Result<()> {
        let mut from = File::from(source);
        let mut to = File::from(target);
        std::io::copy(&mut from, &mut to)?;
        rustix::fs::fchmod(&to, Mode::from_raw_mode(mode))?;
        to.sync_all()?;
        Ok(())
    })()
    .map_err(|err| format!("its content cannot be written ({err})"))
    .and_then(|()| check.map_or(Ok(()), |check| check()))
    .and_then(|()| {
        rustix::fs::renameat(parent, &temp, parent, name)
            .map_err(|err| format!("it cannot be put in place ({err})"))
    });
    written.inspect_err(|_| {
        let _ = rustix::fs::unlinkat(parent, &temp, AtFlags::empty());
    })
}

/// Öffnet das Verzeichnis `rel` im Projekt und legt fehlende Teile an, ohne
/// einem Symlink zu folgen. Die Rechte neuer Verzeichnisse kommen aus der
/// oberen Schicht, ohne Sonderbits.
fn ensure_dir(
    upper: BorrowedFd<'_>,
    lower: BorrowedFd<'_>,
    rel: &Path,
    modes: &mut DirModes,
) -> Result<OwnedFd, String> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
    match open_beneath(lower, rel, flags) {
        Ok(fd) => return Ok(fd),
        Err(Errno::NOENT) => {}
        Err(err) => return Err(parent_problem(rel, err)),
    }
    let mut current = open_beneath(lower, Path::new(""), flags)
        .map_err(|err| format!("the project cannot be opened ({err})"))?;
    let mut walked = PathBuf::new();
    for component in rel.components() {
        let Component::Normal(part) = component else {
            return Err(format!(
                "{} is not a plain relative path",
                display_path(rel)
            ));
        };
        walked.push(part);
        let step = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
        let next = match rustix::fs::openat(&current, part, step, Mode::empty()) {
            Ok(fd) => fd,
            Err(Errno::NOENT) => {
                let mode = upper_mode(upper, &walked).unwrap_or(0o755);
                match rustix::fs::mkdirat(&current, part, Mode::from_raw_mode(mode | 0o700)) {
                    Ok(()) => modes.later(lower, &walked, mode)?,
                    Err(Errno::EXIST) => {}
                    Err(err) => return Err(parent_problem(&walked, err)),
                }
                rustix::fs::openat(&current, part, step, Mode::empty())
                    .map_err(|err| parent_problem(&walked, err))?
            }
            Err(err) => return Err(parent_problem(&walked, err)),
        };
        current = next;
    }
    Ok(current)
}

fn upper_mode(upper: BorrowedFd<'_>, rel: &Path) -> Option<u32> {
    let (parent, name) = (rel.parent()?, rel.file_name()?);
    let dir = open_beneath(
        upper,
        parent,
        OFlags::PATH | OFlags::DIRECTORY | OFlags::CLOEXEC,
    )
    .ok()?;
    let stat = rustix::fs::statat(&dir, name, AtFlags::SYMLINK_NOFOLLOW).ok()?;
    (FileType::from_raw_mode(stat.st_mode) == FileType::Directory).then_some(stat.st_mode & 0o777)
}

fn parent_problem(rel: &Path, err: Errno) -> String {
    match err {
        Errno::LOOP | Errno::XDEV => format!(
            "{} in the project is a symbolic link or leaves the project, and nothing is \
             written through it",
            display_path(rel)
        ),
        Errno::NOTDIR => format!("{} in the project is not a directory", display_path(rel)),
        other => format!(
            "{} cannot be opened in the project ({other})",
            display_path(rel)
        ),
    }
}

/// Ein Name, den es im Zielverzeichnis nicht gibt; `O_EXCL` und `symlinkat`
/// lehnen ab, falls doch.
fn temp_name() -> OsString {
    OsString::from(format!(
        ".humanitl-apply-{}-{}",
        std::process::id(),
        uuid::Uuid::now_v7().simple()
    ))
}
