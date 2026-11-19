//! Die Masken eines Klons und die Schreibweise (HUM-491).
//!
//! Unter macOS liegt der Klon auf demselben APFS-Volume wie das Projekt
//! (`clonefile` kreuzt keine Volumes), und ein APFS-Volume ist meist
//! unempfindlich für die Schreibweise. Es faltet nach Unicode und ist dazu
//! unempfindlich für die Normalisierung: Laut Apple setzt APFS die
//! Unicode-Faltung und -Normalisierung „according to the Unicode 9.0
//! standard" um und findet einen Namen „with any of its normalization
//! variants" (Apple File System Guide, FAQ); die Referenz nennt für den Hash
//! eines Verzeichniseintrags NFD und die Merkmale
//! `APFS_INCOMPAT_CASE_INSENSITIVE` und
//! `APFS_INCOMPAT_NORMALIZATION_INSENSITIVE` (Apple File System Reference,
//! `j_drec_hashed_key_t`), aber keine Tabelle der Faltung. Ein Vergleich der
//! Namen in ASCII verfehlt also `.git/hoo\u{212a}s` (Kelvin-Zeichen) und
//! `.\u{17f}sh` (langes s) ebenso wie `cafe\u{301}` neben `caf\u{e9}`.
//!
//! Statt APFS nachzubauen, fragt der Scan das Volume selbst: In jedem
//! Verzeichnis auf dem Weg zu einer Maske schlägt er den Namen der Maske
//! nach, ohne einem Symlink zu folgen, und vergleicht Gerät und Inode des
//! Gefundenen mit denen jedes Eintrags. Nennt das Nachschlagen der Maske
//! einen Eintrag, ist er die Maske, wie immer er geschrieben ist; die
//! Übernahme schlüge seinen Namen im Projekt auf demselben Volume genauso
//! nach. Auf einem Volume, das die Schreibweise unterscheidet, findet das
//! Nachschlagen nur den genauen Namen, und dort ist eine andere Schreibweise
//! auch im Projekt ein anderer Eintrag. Ein harter Verweis unter einem
//! anderen Namen gilt ebenfalls als Maske; das ist die sichere Seite.
//!
//! Für die Seite des Projekts gilt dasselbe beim Löschen: Hat der Mensch
//! eine Maske in anderer Schreibweise angelegt, entfernt `strip_masks` sie
//! aus dem Klon (das Nachschlagen faltet), und ihr Fehlen dort darf keine
//! Löschung werden ([`ProjectMasks`]). Ein harter Verweis, der sich im
//! Projekt den Inode mit einer Maske teilt, wird ebenso nicht als Löschung
//! gelistet; auch das ist die sichere Seite. Lässt sich ein Eintrag des
//! Projekts nicht nachschlagen, wird keine Löschung gelistet, und die Liste
//! ist unvollständig.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::os::fd::{AsFd as _, BorrowedFd, OwnedFd};
use std::path::{Component, Path, PathBuf};

use rustix::fs::{AtFlags, Mode, OFlags};
use rustix::io::Errno;

/// Gerät und Inode eines Eintrags.
pub(super) type EntryId = (u64, u64);

/// Gerät und Inode aus einem `stat`.
pub(super) fn entry_id(stat: &rustix::fs::Stat) -> EntryId {
    // `st_dev` ist unter macOS ein `i32`. Die Bits werden unverändert
    // übernommen: Ein Abschneiden negativer Werte auf einen gemeinsamen
    // Ersatzwert machte zwei Geräte gleich.
    #[cfg(target_os = "macos")]
    let dev = u64::from(stat.st_dev.cast_unsigned());
    #[cfg(not(target_os = "macos"))]
    #[allow(clippy::useless_conversion)]
    let dev = u64::from(stat.st_dev);
    #[allow(clippy::useless_conversion)]
    let ino = u64::from(stat.st_ino);
    (dev, ino)
}

/// Schlägt einen Namen in einem Ordner des Projekts nach, ohne einem
/// Symlink zu folgen; für Tests austauschbar.
pub(super) type ProjectLookup = fn(BorrowedFd<'_>, &OsStr) -> Result<EntryId, Errno>;

/// Der echte [`ProjectLookup`].
pub(super) fn look_up_entry(dir: BorrowedFd<'_>, name: &OsStr) -> Result<EntryId, Errno> {
    rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW).map(|stat| entry_id(&stat))
}

/// Ein Schritt auf dem Weg zu einer Maske: In diesem Verzeichnis steht der
/// Name `part` der Maske `mask` zum Nachschlagen an.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MaskStep {
    mask: usize,
    part: usize,
}

/// Was das Nachschlagen eines Schritts in einem Verzeichnis fand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Hit {
    /// Der gefundene Eintrag, oder der Fehler des Nachschlagens. Ein Fehler
    /// passt auf jeden Eintrag: Was sich nicht nachschlagen lässt, gilt als
    /// Maske, nie als harmlos.
    id: Result<EntryId, Errno>,
    step: MaskStep,
}

/// Wie ein Eintrag zu den Masken steht.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Standing {
    /// Er ist eine Maske.
    pub(super) on_mask: bool,
    /// Er liegt auf dem Weg zu diesen Masken; unter ihm geht es mit diesen
    /// Schritten weiter.
    pub(super) toward: Vec<MaskStep>,
}

/// Die Masken eines Klons als Folgen von Namen.
#[derive(Debug, Default)]
pub(super) struct CloneMasks {
    names: Vec<Vec<OsString>>,
}

/// Die Namen eines relativen Pfads bis zum ersten, der kein Name ist.
fn names_of(mask: &Path) -> Vec<OsString> {
    mask.components()
        .map_while(|part| match part {
            Component::Normal(name) => Some(name.to_owned()),
            _ => None,
        })
        .collect()
}

impl CloneMasks {
    pub(super) fn new(masks: &[PathBuf]) -> Self {
        Self {
            names: masks.iter().map(|mask| names_of(mask)).collect(),
        }
    }

    /// Die Schritte an der Wurzel des Klons: der erste Name jeder Maske.
    pub(super) fn first_steps(&self) -> Vec<MaskStep> {
        (0..self.names.len())
            .filter(|&mask| !self.names[mask].is_empty())
            .map(|mask| MaskStep { mask, part: 0 })
            .collect()
    }

    /// Schlägt den anstehenden Namen jedes Schritts in `dir` nach, ohne
    /// einem Symlink zu folgen. Was es nicht gibt, fällt weg.
    pub(super) fn look_up(&self, dir: BorrowedFd<'_>, steps: &[MaskStep]) -> Vec<Hit> {
        steps
            .iter()
            .filter_map(|&step| {
                let name = self.names[step.mask][step.part].as_os_str();
                match rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(stat) => Some(Hit {
                        id: Ok(entry_id(&stat)),
                        step,
                    }),
                    Err(Errno::NOENT) => None,
                    Err(err) => Some(Hit { id: Err(err), step }),
                }
            })
            .collect()
    }

    /// Der Name, dessen Nachschlagen scheiterte, mit dem Fehler; für den
    /// Grund einer unvollständigen Liste.
    pub(super) fn failed<'h>(&self, hits: &'h [Hit]) -> Option<(&OsStr, &'h Errno)> {
        hits.iter().find_map(|hit| {
            hit.id
                .as_ref()
                .err()
                .map(|err| (self.names[hit.step.mask][hit.step.part].as_os_str(), err))
        })
    }

    /// Wie der Eintrag `id` zu den Masken steht, nach dem, was `hits` in
    /// seinem Verzeichnis fand.
    pub(super) fn standing(&self, hits: &[Hit], id: EntryId) -> Standing {
        let mut standing = Standing::default();
        for hit in hits {
            if hit.id.as_ref().is_ok_and(|found| *found != id) {
                continue;
            }
            let MaskStep { mask, part } = hit.step;
            if part + 1 == self.names[mask].len() {
                standing.on_mask = true;
            } else {
                standing.toward.push(MaskStep {
                    mask,
                    part: part + 1,
                });
            }
        }
        standing
    }
}

/// Gerät und Inode der Masken, die es im Projekt gibt, so wie das
/// Dateisystem des Projekts ihre Namen nachschlägt.
#[derive(Debug)]
pub(super) enum ProjectMasks {
    /// Die gefundenen Masken, mit dem Nachschlagen eines Eintrags.
    Known(HashSet<EntryId>, ProjectLookup),
    /// Das Nachschlagen scheiterte; dann gilt jeder fehlende Eintrag als
    /// Maske, und keine Löschung kommt in die Liste.
    Unknown,
}

impl ProjectMasks {
    /// Schlägt jede Maske in `lower` Name für Name nach, ohne einem Symlink
    /// zu folgen.
    ///
    /// # Errors
    ///
    /// Der Grund, wenn ein Name sich weder finden noch als fehlend erkennen
    /// lässt.
    pub(super) fn find(
        lower: BorrowedFd<'_>,
        masks: &[PathBuf],
        lookup: ProjectLookup,
    ) -> Result<Self, String> {
        let mut found = HashSet::new();
        for mask in masks {
            if let Some(id) = find_one(lower, &names_of(mask)).map_err(|err| {
                format!(
                    "the masked path {} cannot be looked up in the project ({err})",
                    mask.display()
                )
            })? {
                found.insert(id);
            }
        }
        Ok(Self::Known(found, lookup))
    }

    /// Ob der Eintrag `name` in `dir` des Projekts eine Maske ist. Ein
    /// Eintrag, den es nicht mehr gibt, ist keine.
    ///
    /// # Errors
    ///
    /// Jeder andere Fehler des Nachschlagens: Was sich nicht nachschlagen
    /// lässt, gilt nie als harmlos; der Aufrufer listet dann keine Löschung
    /// und macht die Liste unvollständig.
    pub(super) fn holds(&self, dir: BorrowedFd<'_>, name: &OsStr) -> Result<bool, Errno> {
        match self {
            Self::Unknown => Ok(true),
            Self::Known(ids, lookup) => match lookup(dir, name) {
                Ok(id) => Ok(ids.contains(&id)),
                Err(Errno::NOENT) => Ok(false),
                Err(err) => Err(err),
            },
        }
    }
}

/// Gerät und Inode des Eintrags, den die Namen unter `root` nennen; `None`,
/// wo der Weg fehlt, kein Ordner ist oder über einen Symlink führte.
fn find_one(root: BorrowedFd<'_>, names: &[OsString]) -> Result<Option<EntryId>, Errno> {
    let mut opened: Option<OwnedFd> = None;
    for (index, name) in names.iter().enumerate() {
        let dir = opened.as_ref().map_or(root, |fd| fd.as_fd());
        if index + 1 == names.len() {
            return match rustix::fs::statat(dir, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW) {
                Ok(stat) => Ok(Some(entry_id(&stat))),
                Err(Errno::NOENT | Errno::NOTDIR) => Ok(None),
                Err(err) => Err(err),
            };
        }
        match rustix::fs::openat(
            dir,
            name.as_os_str(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => opened = Some(fd),
            Err(Errno::NOENT | Errno::NOTDIR | Errno::LOOP) => return Ok(None),
            Err(err) => return Err(err),
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn masks(list: &[&str]) -> CloneMasks {
        CloneMasks::new(&list.iter().map(PathBuf::from).collect::<Vec<_>>())
    }

    fn hit(id: Result<EntryId, Errno>, mask: usize, part: usize) -> Hit {
        Hit {
            id,
            step: MaskStep { mask, part },
        }
    }

    /// Entschieden wird über Gerät und Inode, nie über den Namen: Ein
    /// Eintrag, den das Nachschlagen von `.git` fand, geht weiter Richtung
    /// `hooks`, einer mit anderem Inode nicht, und der letzte Name ist die
    /// Maske.
    #[test]
    fn standing_follows_the_identity_not_the_name() {
        let set = masks(&[".git/hooks", ".env"]);
        assert_eq!(set.first_steps().len(), 2);
        let hits = [hit(Ok((1, 10)), 0, 0), hit(Ok((1, 20)), 1, 0)];
        let git = set.standing(&hits, (1, 10));
        assert!(!git.on_mask);
        assert_eq!(git.toward, vec![MaskStep { mask: 0, part: 1 }]);
        assert!(set.standing(&hits, (1, 20)).on_mask);
        assert_eq!(set.standing(&hits, (1, 30)), Standing::default());
        assert_eq!(set.standing(&hits, (2, 10)), Standing::default());
        let below = [hit(Ok((1, 11)), 0, 1)];
        assert!(set.standing(&below, (1, 11)).on_mask);
    }

    /// Ein Nachschlagen, das scheiterte, passt auf jeden Eintrag.
    /// Mutationsbeweis: Ein Fehler, der auf nichts passt, und `on_mask`
    /// bliebe `false`.
    #[test]
    fn a_failed_lookup_counts_as_the_mask() {
        let set = masks(&[".env"]);
        let hits = [hit(Err(Errno::IO), 0, 0)];
        assert!(set.standing(&hits, (9, 9)).on_mask);
        assert_eq!(set.failed(&hits).unwrap().0, OsStr::new(".env"));
    }

    /// Eine Maske ohne Namen hat keinen Schritt; `..` beendet die Namen.
    #[test]
    fn only_plain_names_make_steps() {
        let set = masks(&["", "a/../b"]);
        assert_eq!(set.first_steps(), vec![MaskStep { mask: 1, part: 0 }]);
        assert_eq!(set.names[1].len(), 1);
    }
}
