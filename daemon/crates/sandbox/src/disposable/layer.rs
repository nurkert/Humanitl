//! Die obere Schicht auf dem Host: Anlegen, Messen, Verwerfen, und was vor
//! dem Lesen feststehen muss (HUM-350). Nur unter Linux: Das Overlay kommt
//! von bubblewrap, und der Rest (tmpfs-Prüfung über `statfs`, `/proc`,
//! `openat2`) ist dort zu Hause.

use std::ffi::OsStr;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};
use std::os::unix::fs::DirBuilderExt as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use humanitl_core::diagnostics::codes::{SANDBOX_030, SANDBOX_031, SANDBOX_032};
use humanitl_core::{Diagnostic, FixAction, Severity};
use rustix::fs::{AtFlags, FileType, Mode, OFlags};
use rustix::io::Errno;

use crate::handle::SandboxHandle;
use crate::worktree::{SnapshotLimits, TreeSnapshot};

use super::{Budget, DISPOSABLE_DIR, GUARD_INTERVAL, MAX_DEPTH, OverlayDirs, TmpfsCheck};

/// Die magische Zahl von tmpfs in `statfs(2)`.
const TMPFS_MAGIC: u64 = 0x0102_1994;

/// Eine obere Schicht mit ihrem Arbeitsverzeichnis.
///
/// Sie gehört dem Daemon; die Sandbox sieht sie nur durch das Overlay. Wer
/// sie fallen lässt, ohne [`Layer::discard`] zu rufen, lässt sie im
/// Arbeitsspeicher liegen, bis [`sweep`] beim nächsten Start des Daemons
/// aufräumt. Das ist Absicht: Ein Fehler darf die Änderungen des Menschen
/// nicht stillschweigend wegwerfen.
#[derive(Debug)]
pub struct Layer {
    dir: PathBuf,
    budget: Budget,
    /// Der Stand des Dateisystems beim Anlegen, vor dem Start der Sandbox:
    /// Dagegen misst [`SizeGuard`] das Wachstum, damit nichts, was der
    /// Agent vor dem Anlaufen der Messung schreibt, ungezählt bleibt.
    start: Usage,
}

impl Layer {
    /// Legt eine neue Schicht unter `runtime_dir/disposable/` an.
    ///
    /// `runtime_dir` ist das Laufzeitverzeichnis des Daemons
    /// (`humanitl_config::Paths::runtime_dir`), Rechte `0700`.
    ///
    /// # Errors
    ///
    /// `SANDBOX_030`, wenn das Verzeichnis nicht auf tmpfs liegt oder nicht
    /// anzulegen ist.
    pub fn create(runtime_dir: &Path) -> Result<Self, Diagnostic> {
        Self::create_with(runtime_dir, TmpfsCheck::Require)
    }

    /// Wie [`Layer::create`], mit wählbarer Prüfung auf tmpfs.
    ///
    /// # Errors
    ///
    /// Wie [`Layer::create`].
    pub fn create_with(runtime_dir: &Path, check: TmpfsCheck) -> Result<Self, Diagnostic> {
        let base = runtime_dir.join(DISPOSABLE_DIR);
        // Das Laufzeitverzeichnis legt der Daemon selbst mit `0700` an; hier
        // zählt das eigene Verzeichnis darunter, und das ist privat.
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(runtime_dir)
            .map_err(|err| {
                not_possible(&format!(
                    "{} cannot be created ({err})",
                    runtime_dir.display()
                ))
            })?;
        private_dir(&base)?;
        if check == TmpfsCheck::Require {
            require_tmpfs(&base)?;
        }
        let vfs = rustix::fs::statvfs(&base).map_err(|err| {
            not_possible(&format!(
                "the free space under {} cannot be measured ({err})",
                base.display()
            ))
        })?;
        let budget = Budget::for_free(vfs.f_bavail.saturating_mul(vfs.f_frsize), vfs.f_favail);
        let usage = Usage::of(&base).map_err(|err| {
            not_possible(&format!(
                "the size of {} cannot be measured ({err})",
                base.display()
            ))
        })?;
        let dir = base.join(uuid::Uuid::now_v7().simple().to_string());
        let layer = Self {
            dir,
            budget,
            start: usage,
        };
        for path in [layer.dir.clone(), layer.upper(), layer.work()] {
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&path)
                .map_err(|err| {
                    not_possible(&format!("{} cannot be created ({err})", path.display()))
                })?;
        }
        Ok(layer)
    }

    /// Eine Schicht, deren Verzeichnis es schon gibt; für Tests. Der Stand
    /// des Dateisystems wird jetzt gemessen.
    ///
    /// # Errors
    ///
    /// Der `errno` von `statvfs(2)`.
    pub fn at(dir: impl Into<PathBuf>, budget: Budget) -> Result<Self, Errno> {
        let dir = dir.into();
        let start = Usage::of(&dir)?;
        Ok(Self { dir, budget, start })
    }

    /// Dieselbe Schicht mit einer anderen Grenze; für Tests.
    #[must_use]
    pub const fn with_budget(mut self, budget: Budget) -> Self {
        self.budget = budget;
        self
    }

    /// Der Stand des Dateisystems beim Anlegen.
    #[must_use]
    pub const fn start_usage(&self) -> Usage {
        self.start
    }

    /// Das Verzeichnis der Schicht.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Die obere Schicht.
    #[must_use]
    pub fn upper(&self) -> PathBuf {
        self.dir.join("upper")
    }

    /// Das Arbeitsverzeichnis des Overlays.
    #[must_use]
    pub fn work(&self) -> PathBuf {
        self.dir.join("work")
    }

    /// Die Grenze, die beim Anlegen galt.
    #[must_use]
    pub const fn budget(&self) -> Budget {
        self.budget
    }

    /// Was `bwrap` bekommt.
    #[must_use]
    pub fn dirs(&self) -> OverlayDirs {
        OverlayDirs {
            upper: self.upper(),
            work: self.work(),
        }
    }

    /// Entfernt die Schicht ganz und prüft, dass nichts übrig ist.
    ///
    /// Folgt nie einem Symlink, entfernt Verzeichnisse ohne Rechte (der
    /// Agent darf `chmod 000` und das Overlay legt `work/work` so an) und
    /// hängt Bäume, die tiefer als [`MAX_DEPTH`] sind, flach um, statt
    /// rekursiv zu tief zu steigen.
    ///
    /// # Errors
    ///
    /// `SANDBOX_031`, wenn etwas übrig bleibt; die Schicht bleibt dann in
    /// der Hand des Aufrufers.
    pub fn discard(self) -> Result<(), (Self, Diagnostic)> {
        match remove_dir(&self.dir) {
            Ok(()) if !self.dir.exists() => Ok(()),
            Ok(()) => {
                let diagnostic = discard_failed(&self.dir, "it is still there afterwards");
                Err((self, diagnostic))
            }
            Err(err) => {
                let diagnostic = discard_failed(&self.dir, &err.to_string());
                Err((self, diagnostic))
            }
        }
    }
}

/// Räumt Schichten auf, die ein früherer Daemon liegen ließ (Absturz,
/// `SIGKILL`). Ohne die Grundlage vom Start ist ihre Übernahme nicht sicher;
/// sie werden verworfen. Gibt zurück, wie viele es waren.
#[must_use]
pub fn sweep(runtime_dir: &Path) -> usize {
    let base = runtime_dir.join(DISPOSABLE_DIR);
    let Ok(entries) = std::fs::read_dir(&base) else {
        return 0;
    };
    let mut removed = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        let is_dir = entry
            .file_type()
            .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink());
        if is_dir && remove_dir(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Prüft, dass die Sandbox beendet ist und nichts mehr in ihre Schicht
/// schreiben kann.
///
/// Der PID-Namensraum endet mit seinem PID 1 (dem Shim), und der Kernel
/// beendet dabei jeden Prozess darin; der Mount-Namensraum mit dem Overlay
/// lebt nur, solange ein Prozess darin lebt. Geprüft wird deshalb beides,
/// was der Daemon sehen kann: `bwrap` ist eingesammelt, und der Init-Prozess
/// der Sandbox ist in `/proc` nicht mehr als lebender Prozess zu finden.
///
/// # Errors
///
/// `SANDBOX_031`, solange einer der beiden noch lebt.
pub fn ensure_ended(handle: &SandboxHandle, child_start: Option<u64>) -> Result<(), Diagnostic> {
    if handle.try_wait().is_none() {
        return Err(still_running(&format!(
            "the sandbox {} is still running",
            handle.id
        )));
    }
    if let Some(pid) = handle.child_pid()
        && process_lives(Path::new("/proc"), pid, child_start)
    {
        return Err(still_running(&format!(
            "process {pid}, the first process of sandbox {}, is still alive",
            handle.id
        )));
    }
    Ok(())
}

/// Ob unter `proc_root` ein lebender Prozess mit dieser Nummer steht; ein
/// Zombie lebt nicht mehr.
///
/// Mit `start` (Feld 22 von `/proc/<pid>/stat`, beim Start der Sandbox
/// gemerkt) zählt nur der Prozess, der damals diese Nummer trug: Ein neuer
/// Prozess, der die Nummer wiederverwendet, hat eine andere Startzeit.
#[must_use]
pub fn process_lives(proc_root: &Path, pid: u32, start: Option<u64>) -> bool {
    match std::fs::read_to_string(proc_root.join(pid.to_string()).join("stat")) {
        Ok(line) => {
            let fields = stat_fields(&line);
            if matches!(fields.first(), Some(&("Z" | "X" | "x"))) {
                return false;
            }
            match start {
                Some(expected) => start_of(&fields) == Some(expected),
                None => true,
            }
        }
        Err(error) => error.kind() != std::io::ErrorKind::NotFound,
    }
}

/// Die Startzeit eines Prozesses in Ticks seit dem Systemstart (Feld 22 von
/// `/proc/<pid>/stat`); `None`, wenn es ihn nicht gibt.
#[must_use]
pub fn start_ticks(proc_root: &Path, pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(proc_root.join(pid.to_string()).join("stat")).ok()?;
    start_of(&stat_fields(&stat))
}

/// Die Felder hinter dem Namen: Zustand (Feld 3) zuerst. Der Name steht in
/// Klammern und darf selbst Klammern enthalten, gezählt wird ab der letzten.
fn stat_fields(stat: &str) -> Vec<&str> {
    stat.rfind(')')
        .map(|end| stat[end + 1..].split_whitespace().collect())
        .unwrap_or_default()
}

/// Feld 22 ist das zwanzigste hinter dem Namen.
fn start_of(fields: &[&str]) -> Option<u64> {
    fields.get(19).and_then(|field| field.parse().ok())
}

fn still_running(why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_031, Severity::Error)
        .why(format!(
            "{why}; the changes of a disposable sandbox are read and applied only after \
             every process of it has ended"
        ))
        .fix(FixAction::CopyCommand("humanitl sandbox stop".to_owned()))
        .build()
}

/// Die Grundlage für die Prüfung beim Übernehmen: das Projektverzeichnis vor
/// dem Start, ohne Hashes (Größe, `mtime`, Rechte und Art genügen, um eine
/// Änderung des Menschen zu bemerken) und ohne übersprungene Namen.
///
/// # Errors
///
/// `SANDBOX_011` aus [`crate::worktree::snapshot`], wenn das Verzeichnis
/// nicht zu öffnen ist.
pub fn baseline(lower: &Path) -> Result<TreeSnapshot, Diagnostic> {
    crate::worktree::snapshot(lower, &baseline_limits())
}

/// Dieselbe Grundlage über dem geöffneten Projektordner, den auch Diff und
/// Übernahme benutzen.
#[must_use]
pub fn baseline_at(lower: BorrowedFd<'_>) -> TreeSnapshot {
    crate::worktree::snapshot_at(lower, &baseline_limits())
}

/// Die Grenzen der Grundlage.
#[must_use]
pub fn baseline_limits() -> SnapshotLimits {
    SnapshotLimits {
        max_entries: crate::worktree::DEFAULT_MAX_ENTRIES,
        hash_max_bytes: 0,
        skip_names: Vec::new(),
        skip_paths: Vec::new(),
    }
}

/// Misst das Wachstum des Dateisystems unter einer Schicht und meldet, wenn
/// es die Grenze überschreitet.
///
/// Gemessen wird `statvfs(2)` des Dateisystems, nicht der Baum: Ein Agent
/// kann ein Verzeichnis mit `chmod 000` vor einem Lauf über den Baum
/// verstecken, nicht aber vor dem Zähler des Dateisystems. Was andere auf
/// demselben tmpfs schreiben, zählt mit; das kann die Sandbox früher
/// beenden, nie später.
#[derive(Debug)]
pub struct SizeGuard {
    stop: Arc<AtomicBool>,
    tripped: Arc<Mutex<Option<Diagnostic>>>,
}

/// Der Stand eines Dateisystems, wie ihn [`SizeGuard`] vergleicht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Belegte Bytes.
    pub bytes: u64,
    /// Belegte Einträge.
    pub inodes: u64,
}

impl Usage {
    /// Der Stand des Dateisystems unter `path`.
    ///
    /// # Errors
    ///
    /// Der `errno` von `statvfs(2)`.
    pub fn of(path: &Path) -> Result<Self, Errno> {
        let vfs = rustix::fs::statvfs(path)?;
        Ok(Self {
            bytes: vfs
                .f_blocks
                .saturating_sub(vfs.f_bfree)
                .saturating_mul(vfs.f_frsize),
            inodes: vfs.f_files.saturating_sub(vfs.f_ffree),
        })
    }

    /// Ob `now` gegenüber `self` mehr gewachsen ist, als `budget` erlaubt.
    #[must_use]
    pub fn over(self, now: Self, budget: Budget) -> Option<String> {
        let bytes = now.bytes.saturating_sub(self.bytes);
        let inodes = now.inodes.saturating_sub(self.inodes);
        if bytes > budget.bytes {
            Some(format!(
                "the upper layer grew by {bytes} bytes, over its cap of {} bytes",
                budget.bytes
            ))
        } else if inodes > budget.inodes {
            Some(format!(
                "the upper layer grew by {inodes} entries, over its cap of {} entries",
                budget.inodes
            ))
        } else {
            None
        }
    }
}

impl SizeGuard {
    /// Startet die Messung für `layer` und ruft `on_exceed` einmal, sobald
    /// die Grenze überschritten ist. Der Aufrufer beendet dort die Sandbox.
    ///
    /// # Errors
    ///
    /// `SANDBOX_030`, wenn sich das Dateisystem nicht messen lässt oder der
    /// Faden nicht startet; ohne Messung startet keine Wegwerf-Sandbox.
    pub fn start(
        layer: &Layer,
        on_exceed: impl FnOnce() + Send + 'static,
    ) -> Result<Self, Diagnostic> {
        Self::start_with(
            layer.dir(),
            layer.start_usage(),
            layer.budget(),
            GUARD_INTERVAL,
            on_exceed,
        )
    }

    /// Wie [`SizeGuard::start`], mit eigenem Verzeichnis, eigenem Stand vor
    /// dem Start, eigener Grenze und eigenem Takt.
    ///
    /// # Errors
    ///
    /// Wie [`SizeGuard::start`].
    pub fn start_with(
        dir: &Path,
        start: Usage,
        budget: Budget,
        interval: Duration,
        on_exceed: impl FnOnce() + Send + 'static,
    ) -> Result<Self, Diagnostic> {
        let stop = Arc::new(AtomicBool::new(false));
        let tripped = Arc::new(Mutex::new(None));
        let (thread_stop, thread_tripped) = (Arc::clone(&stop), Arc::clone(&tripped));
        let dir = dir.to_path_buf();
        std::thread::Builder::new()
            .name("disposable-guard".to_owned())
            .spawn(move || {
                while !thread_stop.load(Ordering::SeqCst) {
                    if let Ok(now) = Usage::of(&dir)
                        && let Some(why) = start.over(now, budget)
                    {
                        let diagnostic = Diagnostic::builder(SANDBOX_032, Severity::Error)
                            .why(format!(
                                "{why}; the sandbox was stopped so the agent cannot fill the \
                                 memory, and what it wrote before waits for apply or discard"
                            ))
                            .fix(FixAction::CopyCommand(
                                "humanitl sandbox changes".to_owned(),
                            ))
                            .build();
                        if let Ok(mut slot) = thread_tripped.lock() {
                            *slot = Some(diagnostic);
                        }
                        on_exceed();
                        return;
                    }
                    std::thread::sleep(interval);
                }
            })
            .map_err(|err| not_possible(&format!("the size guard cannot start ({err})")))?;
        Ok(Self { stop, tripped })
    }

    /// Der Befund, wenn die Grenze gegriffen hat.
    #[must_use]
    pub fn tripped(&self) -> Option<Diagnostic> {
        self.tripped.lock().ok().and_then(|slot| slot.clone())
    }

    /// Beendet die Messung.
    pub fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

impl Drop for SizeGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Prüft, dass `dir` auf tmpfs liegt, die obere Schicht dort also im
/// Arbeitsspeicher bliebe (HUM-350).
///
/// Gibt es `dir` noch nicht, zählt das nächste Verzeichnis darüber, das es
/// gibt: Das Laufzeitverzeichnis legt erst die Schicht an, und die Probe vor
/// dem Start (HUM-357) soll es nicht schon anlegen.
///
/// # Errors
///
/// `SANDBOX_030`, wenn das Dateisystem kein tmpfs ist oder sich nicht
/// befragen lässt.
pub fn require_tmpfs(dir: &Path) -> Result<(), Diagnostic> {
    let existing = dir
        .ancestors()
        .find(|candidate| candidate.exists())
        .unwrap_or(dir);
    let stat = rustix::fs::statfs(existing).map_err(|err| {
        not_possible(&format!(
            "the file system under {} cannot be inspected ({err})",
            existing.display()
        ))
    })?;
    if is_tmpfs(&stat) {
        return Ok(());
    }
    Err(Diagnostic::builder(SANDBOX_030, Severity::Blocking)
        .why(format!(
            "{} is not on tmpfs, so the upper layer of a disposable sandbox would \
             not live in RAM and could outlive the session on disk",
            dir.display()
        ))
        .fix(FixAction::SetEnv {
            key: "XDG_RUNTIME_DIR".to_owned(),
            value: format!("/run/user/{}", rustix::process::getuid().as_raw()),
        })
        .build())
}

pub(crate) fn is_tmpfs(stat: &rustix::fs::StatFs) -> bool {
    #[allow(clippy::useless_conversion, clippy::unnecessary_fallible_conversions)]
    let kind = u64::try_from(stat.f_type).ok();
    kind == Some(TMPFS_MAGIC)
}

/// Legt ein Verzeichnis `0700` an, wenn es fehlt, und prüft, dass es ein
/// Verzeichnis dieses Nutzers ohne Rechte für andere ist.
fn private_dir(path: &Path) -> Result<(), Diagnostic> {
    match std::fs::DirBuilder::new().mode(0o700).create(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(err) => {
            return Err(not_possible(&format!(
                "{} cannot be created ({err})",
                path.display()
            )));
        }
    }
    check_private(path)
}

/// Prüft, ohne etwas anzulegen, dass `path` ein Verzeichnis dieses Nutzers
/// ohne Rechte für andere ist (HUM-350); die Probe vor dem Start fragt es,
/// wenn es das Verzeichnis schon gibt (HUM-357).
fn check_private(path: &Path) -> Result<(), Diagnostic> {
    use std::os::unix::fs::MetadataExt as _;

    let meta = std::fs::symlink_metadata(path)
        .map_err(|err| not_possible(&format!("{} cannot be inspected ({err})", path.display())))?;
    let mine = meta.uid() == rustix::process::getuid().as_raw();
    if !meta.is_dir() || !mine || meta.mode() & 0o077 != 0 {
        return Err(not_possible(&format!(
            "{} is not a private directory of this user (a directory with mode 0700 is \
             needed, so no one else reaches the upper layer)",
            path.display()
        )));
    }
    Ok(())
}

/// Was [`Layer::create`] am Laufzeitverzeichnis verlangt, nur gelesen
/// (HUM-357): `dir` (das Verzeichnis der Schichten) liegt auf tmpfs, ist,
/// wenn es das schon gibt, ein privates Verzeichnis dieses Nutzers, und
/// sonst lässt sich unter dem nächsten vorhandenen Verzeichnis darüber
/// anlegen. Legt nichts an und hält keinen Deskriptor.
///
/// # Errors
///
/// `SANDBOX_030` mit der ersten fehlenden Bedingung.
pub fn probe_layer_dir(dir: &Path) -> Result<(), Diagnostic> {
    require_tmpfs(dir)?;
    if std::fs::symlink_metadata(dir).is_ok() {
        return check_private(dir);
    }
    let existing = dir
        .ancestors()
        .find(|candidate| candidate.exists())
        .unwrap_or(dir);
    rustix::fs::access(
        existing,
        rustix::fs::Access::WRITE_OK | rustix::fs::Access::EXEC_OK,
    )
    .map_err(|err| {
        not_possible(&format!(
            "{} cannot be created, {} is not writable ({err})",
            dir.display(),
            existing.display()
        ))
    })
}

fn not_possible(why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_030, Severity::Blocking)
        .why(format!("{why}; the disposable sandbox does not start"))
        .fix(FixAction::ChangeSetting {
            key: "sandbox.mode".to_owned(),
            value: "direct".to_owned(),
        })
        .build()
}

fn discard_failed(dir: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_031, Severity::Error)
        .why(format!(
            "the upper layer {} could not be removed: {why}",
            dir.display()
        ))
        .fix(FixAction::CopyCommand(format!(
            "chmod -R u+rwx {0} && rm -rf {0}",
            crate::bwrap_args::shell_quote(&dir.to_string_lossy())
        )))
        .build()
}

/// Entfernt ein Verzeichnis samt Inhalt, ohne einem Symlink zu folgen.
fn remove_dir(path: &Path) -> std::io::Result<()> {
    let Some(name) = path.file_name() else {
        return Err(std::io::Error::other("no name to remove"));
    };
    let parent = path.parent().unwrap_or_else(|| Path::new("/"));
    let parent = rustix::fs::open(
        parent,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    remove_tree_at(parent.as_fd(), name)?;
    Ok(())
}

/// Entfernt `name` unter `parent` samt allem darunter, ohne einem Symlink zu
/// folgen.
///
/// Rekursiv bis [`MAX_DEPTH`]; darunter werden Unterverzeichnisse nach oben
/// in `parent` umgehängt und danach abgearbeitet, damit ein beliebig tiefer
/// Baum weder den Stapel noch die Deskriptoren erschöpft.
///
/// Nur für Bäume, die dem Daemon gehören (die obere Schicht beim Verwerfen
/// und Aufräumen). Im Projekt des Menschen löscht die Übernahme nie hiermit,
/// sondern Eintrag für Eintrag gegen den Baum der Vorprüfung
/// (`apply::remove_checked`); ohne diesen Baum gibt es dort nur ein
/// einzelnes `unlinkat`.
pub(crate) fn remove_tree_at(parent: BorrowedFd<'_>, name: &OsStr) -> std::io::Result<()> {
    let mut queue: Vec<std::ffi::OsString> = vec![name.to_os_string()];
    let mut counter: u64 = 0;
    while let Some(next) = queue.pop() {
        remove_one(parent, parent, &next, 0, &mut queue, &mut counter)?;
    }
    Ok(())
}

fn remove_one(
    top: BorrowedFd<'_>,
    parent: BorrowedFd<'_>,
    name: &OsStr,
    depth: usize,
    queue: &mut Vec<std::ffi::OsString>,
    counter: &mut u64,
) -> std::io::Result<()> {
    let stat = match rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Ok(()),
        Err(err) => return Err(err.into()),
    };
    if FileType::from_raw_mode(stat.st_mode) != FileType::Directory {
        return unlink(parent, name, AtFlags::empty());
    }
    let dir = open_dir_forcing(parent, name)?;
    loop {
        let (names, more) = crate::worktree::dir_names(dir.as_fd(), 4096)?;
        for child in &names {
            if depth + 1 >= MAX_DEPTH {
                // Flach umhängen: in `top` unter einem Namen, den es dort
                // nicht gibt, und später von oben abarbeiten.
                let child_stat = rustix::fs::statat(dir.as_fd(), child, AtFlags::SYMLINK_NOFOLLOW)?;
                if FileType::from_raw_mode(child_stat.st_mode) == FileType::Directory {
                    queue.push(flatten(dir.as_fd(), child, top, counter)?);
                    continue;
                }
            }
            remove_one(top, dir.as_fd(), child, depth + 1, queue, counter)?;
        }
        if !more {
            break;
        }
    }
    unlink(parent, name, AtFlags::REMOVEDIR)
}

/// Hängt `child` unter einem freien Namen nach `top` um. `RENAME_NOREPLACE`
/// ersetzt nie, was dort schon liegt, auch kein leeres Verzeichnis gleichen
/// Namens; bei `EEXIST` kommt der nächste Name.
fn flatten(
    dir: BorrowedFd<'_>,
    child: &OsStr,
    top: BorrowedFd<'_>,
    counter: &mut u64,
) -> std::io::Result<std::ffi::OsString> {
    loop {
        *counter += 1;
        let flat =
            std::ffi::OsString::from(format!(".humanitl-flat-{}-{counter}", std::process::id()));
        match rustix::fs::renameat_with(dir, child, top, &flat, rustix::fs::RenameFlags::NOREPLACE)
        {
            Ok(()) => return Ok(flat),
            Err(Errno::EXIST) => {}
            Err(err) => return Err(err.into()),
        }
    }
}

fn unlink(parent: BorrowedFd<'_>, name: &OsStr, flags: AtFlags) -> std::io::Result<()> {
    match rustix::fs::unlinkat(parent, name, flags) {
        Ok(()) | Err(Errno::NOENT) => Ok(()),
        Err(err) => Err(err.into()),
    }
}

/// Öffnet ein Verzeichnis unter `parent` zum Lesen; fehlt das Leserecht,
/// bekommt es vorher `0700`. Folgt nie einem Symlink: geöffnet wird mit
/// `O_NOFOLLOW | O_DIRECTORY`, und die Rechte setzt [`chmod_nofollow`] am
/// Inode, den es gerade geöffnet hat.
pub(crate) fn open_dir_forcing(parent: BorrowedFd<'_>, name: &OsStr) -> std::io::Result<OwnedFd> {
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    match rustix::fs::openat(parent, name, flags, Mode::empty()) {
        Ok(fd) => Ok(fd),
        Err(Errno::ACCESS) => {
            chmod_nofollow(parent, name, 0o700, FileType::Directory)?;
            Ok(rustix::fs::openat(parent, name, flags, Mode::empty())?)
        }
        Err(err) => Err(err.into()),
    }
}

/// Setzt die Rechte von `name` unter `parent`, ohne einem Symlink zu folgen.
///
/// `chmodat` folgt einem Symlink, und `AT_SYMLINK_NOFOLLOW` gibt es dafür
/// erst mit `fchmodat2`. Deshalb wird der Eintrag mit `O_PATH | O_NOFOLLOW`
/// geöffnet, seine Art am Deskriptor geprüft und der Inode über
/// `/proc/self/fd` geändert: Ein Tausch gegen einen Symlink zwischen Prüfung
/// und Änderung trifft den geöffneten Inode, nie das Ziel des Symlinks.
///
/// # Errors
///
/// `EINVAL`, wenn der Eintrag nicht von der Art `kind` ist (ein Symlink
/// eingeschlossen), sonst der `errno` des Kernels.
pub(crate) fn chmod_nofollow(
    parent: BorrowedFd<'_>,
    name: &OsStr,
    mode: u32,
    kind: FileType,
) -> Result<(), Errno> {
    use std::os::fd::AsRawFd as _;

    let mut flags = OFlags::PATH | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    if kind == FileType::Directory {
        flags |= OFlags::DIRECTORY;
    }
    let fd = rustix::fs::openat(parent, name, flags, Mode::empty())?;
    let stat = rustix::fs::fstat(&fd)?;
    if FileType::from_raw_mode(stat.st_mode) != kind {
        return Err(Errno::INVAL);
    }
    rustix::fs::chmod(
        format!("/proc/self/fd/{}", fd.as_raw_fd()),
        Mode::from_raw_mode(mode),
    )
}
