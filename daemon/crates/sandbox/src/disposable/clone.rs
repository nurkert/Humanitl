//! Die Schicht unter macOS: ein APFS-Klon des Projekts (HUM-374).
//!
//! Der Daemon klont das geöffnete Projekt mit `fclonefileat(2)` nach
//! `<Laufzeitverzeichnis>/disposable/<id>/upper`. Der Klon teilt die Blöcke
//! mit dem Original, bis jemand schreibt; Seatbelt bekommt ihn als das
//! beschreibbare Projekt, das echte Projekt ist für den Agenten weder lesbar
//! noch schreibbar (`SeatbeltPolicy::sealed`).
//!
//! Das Verzeichnis liegt neben den Sitzungsverzeichnissen, nicht in ihnen:
//! Die Sitzung endet mit der Sandbox, die Änderungen warten darüber hinaus,
//! bis der Mensch übernommen oder verworfen hat.
//!
//! **Masken.** `clonefile` kopiert alles, auch `.env` und `.git/config`.
//! Deshalb entfernt der Daemon jede vorhandene Maske aus dem Klon, bevor die
//! Sandbox startet: Was dort nicht liegt, kann keine Lücke im Profil
//! preisgeben. Das Profil verbietet dieselben Pfade im Klon zusätzlich, damit
//! der Agent an ihrer Stelle nichts anlegen kann, und `scan_clone` zeigt ihr
//! Fehlen nicht als Löschung. Das gilt für jede eingestellte Maske, auch für
//! eine, die es beim Start nicht gibt. Führt der Weg zu einer Maske über einen
//! Symlink, hielte sie hier nicht; der Klon gilt als gescheitert.

use std::ffi::{CStr, CString, OsStr};
use std::os::fd::{AsRawFd as _, BorrowedFd, OwnedFd};
use std::os::raw::{c_char, c_int, c_void};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Path, PathBuf};

use humanitl_core::Diagnostic;
use humanitl_core::diagnostics::codes::SANDBOX_030;
use humanitl_core::{FixAction, Severity};
use rustix::fs::OFlags;
use rustix::io::Errno;

use super::layer::remove_tree_at;
use crate::worktree::open_beneath;

/// `ACL_TYPE_EXTENDED` aus `<sys/acl.h>`.
const ACL_TYPE_EXTENDED: c_int = 0x0100;

/// `CLONE_NOFOLLOW` aus `<sys/clonefile.h>`.
const CLONE_NOFOLLOW: u32 = 0x0001;
/// `CLONE_NOOWNERCOPY` aus `<sys/clonefile.h>`: Der Klon gehört dem Daemon.
const CLONE_NOOWNERCOPY: u32 = 0x0002;

// Eine eigene Deklaration statt der Crate `libc`: Die Funktion steht in
// `libSystem`, gegen das jedes Programm unter macOS gebunden ist, und diese
// Crate bekommt so keine neue Abhängigkeit.
#[allow(unsafe_code)]
unsafe extern "C" {
    fn lchflags(path: *const c_char, flags: u32) -> c_int;
    fn acl_init(count: c_int) -> *mut c_void;
    fn acl_set_link_np(path: *const c_char, kind: c_int, acl: *mut c_void) -> c_int;
    fn acl_free(acl: *mut c_void) -> c_int;
    fn fclonefileat(srcfd: c_int, dst_dirfd: c_int, dst: *const c_char, flags: u32) -> c_int;
}

/// Klont das geöffnete Verzeichnis `source` als `name` unter `parent`.
fn clone_dir(source: BorrowedFd<'_>, parent: BorrowedFd<'_>, name: &CStr) -> Result<(), Errno> {
    // SAFETY: Beide Deskriptoren sind für die Dauer des Aufrufs geliehen,
    // `name` ist nullterminiert und wird nur gelesen.
    #[allow(unsafe_code)]
    let result = unsafe {
        fclonefileat(
            source.as_raw_fd(),
            parent.as_raw_fd(),
            name.as_ptr(),
            CLONE_NOFOLLOW | CLONE_NOOWNERCOPY,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error()
            .raw_os_error()
            .map_or(Errno::IO, Errno::from_raw_os_error))
    }
}

/// Nimmt Dateiflags und ACL von `path`, ohne einem Symlink zu folgen.
/// Scheitert das, zeigt es das folgende Löschen.
fn clear(path: &Path) {
    let Ok(path) = CString::new(path.as_os_str().as_bytes()) else {
        return;
    };
    // SAFETY: `path` ist nullterminiert und wird nur gelesen; `acl_init`
    // liefert eine leere Liste oder null, und nur eine echte wird benutzt
    // und mit `acl_free` zurückgegeben.
    #[allow(unsafe_code)]
    unsafe {
        lchflags(path.as_ptr(), 0);
        let acl = acl_init(1);
        if !acl.is_null() {
            acl_set_link_np(path.as_ptr(), ACL_TYPE_EXTENDED, acl);
            acl_free(acl);
        }
    }
}

/// Nimmt Dateiflags und ACLs von `name` und seinem Verzeichnis; beides kann
/// das Löschen verbieten.
pub(crate) fn unlock(parent: BorrowedFd<'_>, name: &OsStr) {
    if let Ok(dir) = fd_path(parent) {
        clear(&dir);
        clear(&dir.join(name));
    }
}

/// Der Name des Dateisystems aus `fstatfs(2)`.
fn volume_kind(fd: BorrowedFd<'_>) -> Result<String, Errno> {
    let stat = rustix::fs::fstatfs(fd)?;
    let name: Vec<u8> = stat
        .f_fstypename
        .iter()
        .take_while(|&&byte| byte != 0)
        .map(|&byte| byte.cast_unsigned())
        .collect();
    Ok(String::from_utf8_lossy(&name).into_owned())
}

/// Der Befund, wenn der Klon nicht geht: Ein ausdrückliches `disposable`
/// lehnt damit ab, die Vorgabe `auto` läuft direkt mit `SANDBOX_033`.
pub(crate) fn refused(why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_030, Severity::Blocking)
        .why(format!("{why}; the disposable sandbox does not start"))
        .fix(FixAction::ChangeSetting {
            key: "sandbox.mode".to_owned(),
            value: "direct".to_owned(),
        })
        .build()
}

/// Warum dieser Volume-Typ keinen Klon trägt; `None` für APFS.
pub(crate) fn volume_problem(kind: &str, project: &Path) -> Option<String> {
    (kind != "apfs").then(|| {
        format!(
            "{} lies on a {kind} volume; a disposable sandbox on macOS clones the project, \
             and only APFS can do that",
            project.display()
        )
    })
}

/// Warum `clonefile` scheiterte, in den Worten des Menschen.
pub(crate) fn clone_problem(err: Errno, project: &Path, layer: &Path) -> String {
    match err {
        Errno::XDEV => format!(
            "{} and the layer directory {} lie on different volumes, and a clone cannot \
             cross volumes",
            project.display(),
            layer.display()
        ),
        Errno::ACCESS | Errno::PERM => format!(
            "{} cannot be cloned ({err}), usually because a file in it is not readable; \
             clonefile takes the whole tree or nothing",
            project.display()
        ),
        Errno::NOTSUP => format!("the volume of {} cannot clone ({err})", project.display()),
        Errno::NOSPC | Errno::DQUOT => format!(
            "the volume of {} has no room for the clone ({err})",
            project.display()
        ),
        other => format!("{} cannot be cloned ({other})", project.display()),
    }
}

/// Klont `lower` nach `parent`/`name` und entfernt die Masken aus dem Klon.
///
/// `layer` ist der Pfad von `parent`, `project` der von `lower`, beide nur
/// für den Befund und die Prüfung, dass die Schicht nicht im Projekt liegt.
/// `masks` sind Pfade relativ zum Projekt.
///
/// # Errors
///
/// `SANDBOX_030`, wenn das Volume kein APFS ist, der Klon scheitert oder
/// eine Maske nicht entfernt werden kann. Was schon im Klon liegt, räumt der
/// Aufrufer mit der Schicht weg.
pub(crate) fn clone_project(
    lower: BorrowedFd<'_>,
    parent: BorrowedFd<'_>,
    name: &OsStr,
    project: &Path,
    layer: &Path,
    masks: &[PathBuf],
) -> Result<(), Diagnostic> {
    if layer.starts_with(project) {
        return Err(refused(&format!(
            "the layer directory {} lies inside the project {}",
            layer.display(),
            project.display()
        )));
    }
    let kind = volume_kind(lower).map_err(|err| {
        refused(&format!(
            "the volume of {} cannot be inspected ({err})",
            project.display()
        ))
    })?;
    if let Some(why) = volume_problem(&kind, project) {
        return Err(refused(&why));
    }
    let c_name = CString::new(name.as_bytes())
        .map_err(|_err| refused("the name of the layer holds a NUL byte"))?;
    clone_dir(lower, parent, &c_name)
        .map_err(|err| refused(&clone_problem(err, project, layer)))?;
    let clone = open_beneath(
        parent,
        Path::new(name),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
    )
    .map_err(|err| refused(&format!("the clone cannot be opened ({err})")))?;
    strip_masks(&clone, masks)
}

/// Entfernt jede Maske aus dem Klon, ohne einem Symlink zu folgen.
fn strip_masks(clone: &OwnedFd, masks: &[PathBuf]) -> Result<(), Diagnostic> {
    use std::os::fd::AsFd as _;

    for mask in masks {
        let (Some(parent), Some(name)) = (mask.parent(), mask.file_name()) else {
            continue;
        };
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        let dir = match open_beneath(clone.as_fd(), parent, flags) {
            Ok(dir) => dir,
            // Eine eingestellte Maske, die es hier nicht gibt.
            Err(Errno::NOENT) => continue,
            // `O_NOFOLLOW | O_DIRECTORY` sagt dasselbe zu einer Datei und zu
            // einem Symlink; nur die Datei ist harmlos.
            Err(Errno::NOTDIR) if !behind_symlink(clone, parent) => continue,
            Err(err) => {
                return Err(refused(&format!(
                    "the masked path {} cannot be removed from the clone, because the way to \
                     it is a symbolic link or cannot be opened ({err})",
                    mask.display()
                )));
            }
        };
        remove_tree_at(dir.as_fd(), name).map_err(|err| {
            refused(&format!(
                "the masked path {} cannot be removed from the clone ({err})",
                mask.display()
            ))
        })?;
    }
    Ok(())
}

/// Ob auf dem Weg `rel` unter `root` ein Symlink liegt; flach zuerst, also
/// ist alles davor ein echter Ordner.
fn behind_symlink(root: &OwnedFd, rel: &Path) -> bool {
    use rustix::fs::{AtFlags, FileType};

    let mut steps: Vec<&Path> = rel.ancestors().collect();
    steps.reverse();
    steps
        .into_iter()
        .filter(|step| !step.as_os_str().is_empty())
        .find_map(|step| {
            rustix::fs::statat(root, step, AtFlags::SYMLINK_NOFOLLOW)
                .ok()
                .map(|stat| FileType::from_raw_mode(stat.st_mode))
                .filter(|kind| *kind != FileType::Directory)
        })
        == Some(FileType::Symlink)
}

/// Wo ein geöffnetes Verzeichnis liegt (`F_GETPATH`).
///
/// # Errors
///
/// Der `errno` des Kernels.
pub fn fd_path(fd: BorrowedFd<'_>) -> Result<PathBuf, Errno> {
    use std::os::unix::ffi::OsStringExt as _;

    let path = rustix::fs::getpath(fd)?;
    Ok(PathBuf::from(std::ffi::OsString::from_vec(
        path.into_bytes(),
    )))
}
