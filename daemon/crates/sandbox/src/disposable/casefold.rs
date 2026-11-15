//! Masken und Schreibweise (HUM-482).
//!
//! Der Scan vergleicht die Pfade der oberen Schicht genau mit den Masken
//! ([`super::scan_masked`]). Das hält nur, solange das Projekt jede
//! Schreibweise als eigenen Namen führt. Ein Ordner mit dem Attribut
//! Casefold (ext4, f2fs, tmpfs, `chattr +F`) oder ein Dateisystem, das
//! Schreibweisen von sich aus gleichsetzt (vfat, exFAT, SMB, HFS, HFS+), führt
//! `.GIT/hooks` und `.git/hooks` als denselben Eintrag. Unter der
//! Kernel-Stufe sieht der Gast davon nichts: Sein virtiofs reicht jeden
//! Namen an den Dateisystemdienst auf dem Host weiter, und erst dessen `openat`
//! faltet; das Overlay im Gast und die obere Schicht auf tmpfs unterscheiden
//! die Schreibweise. Root im Gast könnte so `.GIT/hooks/pre-commit` in die
//! Schicht legen, der Scan sähe keine Maske, und die Übernahme schriebe nach
//! `.git/hooks/pre-commit`. Unter bwrap verweigert overlayfs einen gefalteten
//! Ordner selbst (beim Einhängen oder mit `EREMOTE` beim Nachschlagen,
//! gesehen unter Linux 7.3); darauf verlässt sich Humanitl nicht.
//!
//! Falten statt verweigern taugt nicht: ext4 und tmpfs falten nach Unicode
//! und normalisieren dazu (das Kelvin-Zeichen U+212A steht für `k`, das
//! lange s U+017F für `s`), ein Vergleich in ASCII wie bei den Klon-Masken
//! unter macOS ließe solche Namen durch. Deshalb startet eine
//! Wegwerf-Sandbox nicht, wenn einer der Ordner, in denen ein Name einer
//! Maske nachgeschlagen wird, Schreibweisen gleichsetzt: die Wurzel des
//! Projekts und jeder vorhandene Ordner auf dem Weg zu einer Maske. Was darin
//! sonst liegt, spielt keine Rolle; ein Ordner, den die Übernahme erst
//! anlegt, erbt das Attribut von einem Ordner, der schon geprüft ist. Das
//! Attribut setzen kann nur der Eigentümer, und nur auf einem leeren Ordner;
//! der Agent erreicht das Projekt in diesem Modus nie schreibend. Der
//! Modus `direct` ist nicht betroffen: Dort hängen die Masken auf dem Host
//! über genau den Einträgen, und das gefaltete Nachschlagen trifft sie.

use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::path::{Component, Path, PathBuf};

use humanitl_core::diagnostics::codes::{SANDBOX_030, SANDBOX_031};
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::fs::{AtFlags, Mode, OFlags};
use rustix::io::Errno;

/// `FS_CASEFOLD_FL` aus `linux/fs.h`; rustix führt es nicht als Namen.
const FS_CASEFOLD_FL: u32 = 0x4000_0000;

/// Dateisysteme, die Schreibweisen von sich aus gleichsetzen, mit ihrer
/// magischen Zahl in `statfs(2)`.
const FOLDING_FILESYSTEMS: &[(u64, &str)] = &[
    (0x4d44, "vfat"),
    (0x2011_bab0, "exfat"),
    (0x482b, "hfsplus"),
    (0x4244, "hfs"),
    (0xff53_4d42, "cifs"),
    (0xfe53_4d42, "smb2"),
];

/// Liest, ob ein Ordner das Attribut Casefold trägt; für Tests austauschbar.
pub(crate) type CasefoldReader<'a> = &'a dyn Fn(BorrowedFd<'_>) -> Result<bool, Errno>;

/// Schlägt einen Namen in einem Ordner nach, ohne einem Symlink zu folgen,
/// und liefert Gerät und Inode; für Tests austauschbar.
pub(crate) type EntryLookup<'a> = &'a dyn Fn(BorrowedFd<'_>, &OsStr) -> Result<(u64, u64), Errno>;

/// Verweigert die Wegwerf-Sandbox, wenn das geöffnete Projekt eine Maske
/// über die Schreibweise erreichbar machte.
///
/// `lower` ist das geöffnete Projekt, `masks` sind die Masken relativ zu ihm
/// ([`crate::clone_masks`]).
///
/// # Errors
///
/// `SANDBOX_030` mit dem Rückweg auf `direct`, wenn die Wurzel oder ein
/// Ordner auf dem Weg zu einer Maske Schreibweisen gleichsetzt, oder wenn
/// sich das nicht feststellen lässt.
pub fn refuse_case_folding(lower: BorrowedFd<'_>, masks: &[PathBuf]) -> Result<(), Diagnostic> {
    refuse_case_folding_with(lower, masks, &casefold_flag)
}

/// Dieselbe Prüfung vor dem Lesen der Schicht und vor dem Schreiben ins
/// Projekt (HUM-482, Review): Seit dem Start kann der Mensch einen leeren
/// Ordner auf dem Weg zu einer Maske gefaltet oder einen Namen in anderer
/// Schreibweise angelegt haben.
///
/// # Errors
///
/// `SANDBOX_031` mit dem Rückweg auf `humanitl sandbox discard`, aus
/// denselben Gründen wie [`refuse_case_folding`]; dann ist nichts gelesen
/// und nichts geschrieben.
pub fn refuse_case_folding_before_apply(
    lower: BorrowedFd<'_>,
    masks: &[PathBuf],
) -> Result<(), Diagnostic> {
    find_folding(lower, masks, &casefold_flag, &entry_id).map_err(|(why, mask)| {
        Diagnostic::builder(SANDBOX_031, Severity::Error)
            .why(format!(
                "{why}, so a path that differs from the mask {} only in case would reach the \
                 masked path in the project; nothing is read or applied from the layer",
                mask.display()
            ))
            .fix(FixAction::CopyCommand(
                "humanitl sandbox discard".to_owned(),
            ))
            .build()
    })
}

/// Wie [`refuse_case_folding`], mit eigenem Leser des Attributs.
pub(crate) fn refuse_case_folding_with(
    lower: BorrowedFd<'_>,
    masks: &[PathBuf],
    read: CasefoldReader<'_>,
) -> Result<(), Diagnostic> {
    refuse_case_folding_probed(lower, masks, read, &entry_id)
}

/// Wie [`refuse_case_folding_with`], dazu mit eigenem Nachschlagen.
pub(crate) fn refuse_case_folding_probed(
    lower: BorrowedFd<'_>,
    masks: &[PathBuf],
    read: CasefoldReader<'_>,
    lookup: EntryLookup<'_>,
) -> Result<(), Diagnostic> {
    find_folding(lower, masks, read, lookup).map_err(|(why, mask)| {
        Diagnostic::builder(SANDBOX_030, Severity::Blocking)
            .why(format!(
                "{why}, so a path that differs from the mask {} only in case would reach \
                 the masked path in the project; the disposable sandbox does not start",
                mask.display()
            ))
            .fix(FixAction::ChangeSetting {
                key: "sandbox.mode".to_owned(),
                value: "direct".to_owned(),
            })
            .build()
    })
}

/// Der erste Grund und seine Maske, oder nichts.
fn find_folding<'m>(
    lower: BorrowedFd<'_>,
    masks: &'m [PathBuf],
    read: CasefoldReader<'_>,
    lookup: EntryLookup<'_>,
) -> Result<(), (String, &'m Path)> {
    let mut checked = HashSet::new();
    for mask in masks {
        check_mask(lower, mask, read, lookup, &mut checked).map_err(|why| (why, mask.as_path()))?;
    }
    Ok(())
}

/// Geht den Weg zu einer Maske Name für Name, ohne einem Symlink zu folgen.
/// Wo der Weg endet (fehlt, kein Ordner, ein Symlink), legt die Übernahme
/// nichts unter einem gefalteten Ordner an, den es jetzt gibt.
fn check_mask(
    lower: BorrowedFd<'_>,
    mask: &Path,
    read: CasefoldReader<'_>,
    lookup: EntryLookup<'_>,
    checked: &mut HashSet<(u64, u64)>,
) -> Result<(), String> {
    let names: Vec<&OsStr> = mask
        .components()
        .map_while(|part| match part {
            Component::Normal(name) => Some(name),
            _ => None,
        })
        .collect();
    let mut opened: Option<OwnedFd> = None;
    let mut at = PathBuf::new();
    for (index, name) in names.iter().enumerate() {
        let dir = opened.as_ref().map_or(lower, AsFd::as_fd);
        check_dir(dir, &at, read, checked)?;
        let same = same_entry(dir, name, &swapped_case(name), lookup).map_err(|err| {
            format!(
                "whether {} in the project folder has another entry in another case cannot be \
                 checked ({err})",
                shown(&at.join(name))
            )
        })?;
        if same {
            return Err(format!(
                "{} in the project folder names the same entry in another case",
                shown(&at.join(name))
            ));
        }
        if index + 1 == names.len() {
            break;
        }
        match rustix::fs::openat(
            dir,
            *name,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => opened = Some(fd),
            Err(Errno::NOENT | Errno::NOTDIR | Errno::LOOP) => break,
            Err(err) => {
                return Err(format!(
                    "whether {} in the project folder tells cases apart cannot be checked ({err})",
                    shown(&at.join(name))
                ));
            }
        }
        at.push(name);
    }
    Ok(())
}

/// Prüft einen Ordner einmal: Attribut und Dateisystem.
fn check_dir(
    dir: BorrowedFd<'_>,
    rel: &Path,
    read: CasefoldReader<'_>,
    checked: &mut HashSet<(u64, u64)>,
) -> Result<(), String> {
    let unknown = |err: Errno| {
        format!(
            "whether {} in the project folder tells cases apart cannot be checked ({err})",
            shown(rel)
        )
    };
    let stat = rustix::fs::fstat(dir).map_err(unknown)?;
    #[allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]
    let dev = u64::try_from(stat.st_dev).unwrap_or(u64::MAX);
    if !checked.insert((dev, stat.st_ino)) {
        return Ok(());
    }
    if read(dir).map_err(unknown)? {
        return Err(format!(
            "{} in the project folder is case-insensitive (casefold attribute)",
            shown(rel)
        ));
    }
    let fs = rustix::fs::fstatfs(dir).map_err(unknown)?;
    #[allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]
    let kind = u64::try_from(fs.f_type).ok();
    if let Some((_, name)) = FOLDING_FILESYSTEMS
        .iter()
        .find(|(magic, _)| Some(*magic) == kind)
    {
        return Err(format!(
            "{} in the project folder lies on {name}, which treats names that differ only in \
             case as one",
            shown(rel)
        ));
    }
    Ok(())
}

/// Ob ein Ordner das Attribut Casefold trägt. Ein Dateisystem ohne
/// Attribute (`ENOTTY` und Verwandte) trägt es nicht.
fn casefold_flag(dir: BorrowedFd<'_>) -> Result<bool, Errno> {
    match rustix::fs::ioctl_getflags(dir) {
        Ok(flags) => Ok(flags.bits() & FS_CASEFOLD_FL != 0),
        Err(Errno::NOTTY | Errno::INVAL | Errno::OPNOTSUPP | Errno::NOSYS) => Ok(false),
        Err(err) => Err(err),
    }
}

/// Ob zwei Namen in `dir` denselben Eintrag nennen. Fängt ein Dateisystem,
/// das faltet, ohne es zu sagen (FUSE), solange der Name schon da ist; ein
/// harter Verweis unter dem anderen Namen verweigert ebenfalls, und das ist
/// die sichere Seite.
///
/// # Errors
///
/// Jeder Fehler des Nachschlagens außer `ENOENT` (und `ENOTDIR`, wenn der
/// Name kein Ordner ist): Was sich nicht nachschlagen lässt, gilt nie als
/// abwesend.
fn same_entry(
    dir: BorrowedFd<'_>,
    name: &OsStr,
    other: &OsStr,
    lookup: EntryLookup<'_>,
) -> Result<bool, Errno> {
    if name == other {
        return Ok(false);
    }
    let id = |name: &OsStr| match lookup(dir, name) {
        Ok(id) => Ok(Some(id)),
        Err(Errno::NOENT | Errno::NOTDIR) => Ok(None),
        Err(err) => Err(err),
    };
    let first = id(name)?;
    let second = id(other)?;
    Ok(matches!((first, second), (Some(a), Some(b)) if a == b))
}

/// Gerät und Inode eines Namens in `dir`, ohne einem Symlink zu folgen.
fn entry_id(dir: BorrowedFd<'_>, name: &OsStr) -> Result<(u64, u64), Errno> {
    let stat = rustix::fs::statat(dir, name, AtFlags::SYMLINK_NOFOLLOW)?;
    #[allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]
    let dev = u64::try_from(stat.st_dev).unwrap_or(u64::MAX);
    Ok((dev, stat.st_ino))
}

/// Der Name mit vertauschter ASCII-Schreibweise.
fn swapped_case(name: &OsStr) -> OsString {
    OsString::from_vec(
        name.as_bytes()
            .iter()
            .map(|byte| {
                if byte.is_ascii_lowercase() {
                    byte.to_ascii_uppercase()
                } else {
                    byte.to_ascii_lowercase()
                }
            })
            .collect(),
    )
}

/// Ein Pfad im Projekt für einen Satz; die Wurzel heißt `.`.
fn shown(rel: &Path) -> String {
    if rel.as_os_str().is_empty() {
        ".".to_owned()
    } else {
        rel.display().to_string()
    }
}
