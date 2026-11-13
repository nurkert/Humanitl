//! Kernel modules in the guest mode: load the few the door needs, then lock
//! loading for good (HUM-465, ADR-0021, HUM-442 Q6).
//!
//! The guest kernel is the host's own distribution kernel, and vsock is a
//! module there. The shim, PID 1 of the guest, loads `vsock`,
//! `vmw_vsock_virtio_transport_common` and `vmw_vsock_virtio_transport`
//! (and whatever extra list the plan names) from the plan's module directory
//! with `finit_module(2)`, compressed or not, and then writes `1` to
//! `/proc/sys/kernel/modules_disabled`. That switch is one-way until the
//! guest powers off: from then on no process of the guest loads a module,
//! root included. The agent starts long after it.
//!
//! A module is found by its file name below the directory, as `<name>.ko`,
//! `<name>.ko.xz` or `<name>.ko.zst`, `-` and `_` alike, exactly once; a
//! distribution package ships the files without `modules.dep` (depmod runs at
//! install time), so the walk does not rely on it. One that
//! `modules.builtin` lists is part of the kernel and needs no loading, but
//! only when the running kernel shows it under `/sys/module/<name>`; a list
//! that claims more than the kernel has fails closed. The
//! order is the plan's, dependencies first; a failure stops everything, and
//! the guest powers off without the agent (`guest.rs`).

use std::fmt;
use std::fs;
use std::io;
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

/// The modules the door needs, in the order of their dependencies.
pub const DOOR_MODULES: [&str; 3] = [
    "vsock",
    "vmw_vsock_virtio_transport_common",
    "vmw_vsock_virtio_transport",
];

/// The file endings a module may have, uncompressed first.
pub const ENDINGS: [&str; 3] = [".ko", ".ko.xz", ".ko.zst"];

/// How deep the walk goes below the module directory; a distribution puts
/// modules at `kernel/<area>/<subsystem>/...`, four or five levels.
const MAX_DEPTH: usize = 10;

/// How many entries the walk reads at most; a full distribution tree has
/// some ten thousand.
const MAX_ENTRIES: usize = 200_000;

/// `MODULE_INIT_COMPRESSED_FILE` of `<linux/module.h>`: the kernel unpacks
/// the file itself (`CONFIG_MODULE_DECOMPRESS`).
const MODULE_INIT_COMPRESSED_FILE: libc::c_uint = 4;

/// Where the switch lives.
const MODULES_DISABLED: &str = "/proc/sys/kernel/modules_disabled";

/// A module as the plan names it, normalised: `-` becomes `_`, as the kernel
/// itself names modules.
fn normalised(name: &str) -> String {
    name.replace('-', "_")
}

/// What went wrong, with the module it concerns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// A name that cannot be a module: empty, or with a path in it.
    BadName(String),
    /// No file of that name below the directory.
    NotFound(String),
    /// More than one file of that name.
    Ambiguous(String, usize),
    /// The directory cannot be read.
    Directory(String),
    /// `finit_module(2)` refused it.
    Load(String, String),
    /// `modules.builtin` lists it, and the running kernel does not show it.
    BuiltinAbsent(String),
    /// The switch could not be set, or did not read back as `1`.
    Lock(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadName(name) => write!(f, "bad-name:{name}"),
            Self::NotFound(name) => write!(f, "not-found:{name}"),
            Self::Ambiguous(name, count) => write!(f, "ambiguous:{name}:{count}"),
            Self::Directory(why) => write!(f, "directory:{why}"),
            Self::Load(name, why) => write!(f, "load:{name}:{why}"),
            Self::BuiltinAbsent(name) => write!(f, "builtin-absent:{name}"),
            Self::Lock(why) => write!(f, "lock:{why}"),
        }
    }
}

/// How a module ended up in the kernel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Loaded {
    /// Part of the kernel already (`modules.builtin`).
    Builtin,
    /// Loaded from this file.
    File(PathBuf),
    /// Loaded before, by the kernel or an earlier step (`EEXIST`).
    Present,
}

/// The three steps, so that the order can be tested without a kernel.
pub trait Kernel {
    /// Whether `modules.builtin` lists the module.
    fn builtin(&self, name: &str) -> bool;
    /// Whether the running kernel shows the module (`/sys/module/<name>`).
    fn present(&self, name: &str) -> bool;
    /// The one file of the module below the directory.
    fn find(&self, name: &str) -> Result<PathBuf, Error>;
    /// Loads one file; `Ok(false)` when the module was loaded already.
    fn load(&mut self, path: &Path) -> Result<bool, String>;
    /// Sets the switch and reads it back.
    fn lock(&mut self) -> Result<(), String>;
}

/// Loads the door's modules and then `extra`, in that order, and locks
/// loading. Stops at the first failure: then nothing is locked, and the
/// caller powers the guest off without starting anything.
///
/// # Errors
///
/// The first [`Error`].
pub fn load_and_lock(
    kernel: &mut dyn Kernel,
    extra: &[String],
) -> Result<Vec<(String, Loaded)>, Error> {
    let mut done = Vec::new();
    let names = DOOR_MODULES
        .iter()
        .map(|name| (*name).to_owned())
        .chain(extra.iter().cloned());
    for name in names {
        if name.is_empty() || name.contains(['/', '\0']) || name == "." || name == ".." {
            return Err(Error::BadName(name));
        }
        let name = normalised(&name);
        if done.iter().any(|(seen, _)| *seen == name) {
            continue;
        }
        let loaded = if kernel.builtin(&name) {
            if !kernel.present(&name) {
                return Err(Error::BuiltinAbsent(name));
            }
            Loaded::Builtin
        } else {
            let path = kernel.find(&name)?;
            match kernel.load(&path) {
                Ok(true) => Loaded::File(path),
                Ok(false) => Loaded::Present,
                Err(why) => return Err(Error::Load(name, why)),
            }
        };
        done.push((name, loaded));
    }
    kernel.lock().map_err(Error::Lock)?;
    Ok(done)
}

/// The evidence of a successful [`load_and_lock`]: every module and how it
/// came in, then the lock.
#[must_use]
pub fn evidence(done: &[(String, Loaded)]) -> String {
    let mut parts: Vec<String> = done
        .iter()
        .map(|(name, loaded)| match loaded {
            Loaded::Builtin => format!("{name}=builtin"),
            Loaded::File(_) => format!("{name}=loaded"),
            Loaded::Present => format!("{name}=present"),
        })
        .collect();
    parts.push("modules_disabled=1".to_owned());
    parts.join(",")
}

/// The module name a file carries, when it is a module file.
#[must_use]
pub fn module_name(file_name: &str) -> Option<String> {
    ENDINGS
        .iter()
        .rev()
        .find_map(|ending| file_name.strip_suffix(ending))
        .filter(|stem| !stem.is_empty())
        .map(normalised)
}

/// The kernel of this guest, with the module directory of the plan.
pub struct Running {
    /// The names `modules.builtin` lists; empty when the file is missing.
    builtin: Vec<String>,
    /// Every module file below the directory, by name.
    files: Vec<(String, PathBuf)>,
}

impl Running {
    /// The kernel with modules from `dir`; reads the directory once.
    ///
    /// # Errors
    ///
    /// [`Error::Directory`] when the directory cannot be read.
    pub fn open(dir: &Path) -> Result<Self, Error> {
        let builtin = fs::read_to_string(dir.join("modules.builtin"))
            .map(|text| {
                text.lines()
                    .filter_map(|line| line.rsplit('/').next())
                    .filter_map(module_name)
                    .collect()
            })
            .unwrap_or_default();
        let files = walk(dir).map_err(Error::Directory)?;
        Ok(Self { builtin, files })
    }
}

/// Every module file below `dir`, depth first, bounded.
fn walk(dir: &Path) -> Result<Vec<(String, PathBuf)>, String> {
    let mut found = Vec::new();
    let mut stack = vec![(dir.to_path_buf(), 0usize)];
    let mut entries = 0usize;
    // The directory itself must be readable; below it, an unreadable
    // directory only hides what is in it, and a module missing that way is
    // `NotFound`.
    fs::read_dir(dir).map_err(|err| format!("{}:{err}", dir.display()))?;
    while let Some((path, depth)) = stack.pop() {
        let Ok(list) = fs::read_dir(&path) else {
            continue;
        };
        for entry in list.filter_map(Result::ok) {
            entries += 1;
            if entries > MAX_ENTRIES {
                return Err(format!("more-than-{MAX_ENTRIES}-entries"));
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let child = entry.path();
            if kind.is_dir() && depth < MAX_DEPTH {
                stack.push((child, depth + 1));
            } else if kind.is_file()
                && let Some(name) = entry.file_name().to_str().and_then(module_name)
            {
                found.push((name, child));
            }
        }
    }
    Ok(found)
}

impl Kernel for Running {
    fn builtin(&self, name: &str) -> bool {
        self.builtin.iter().any(|known| known == name)
    }

    fn present(&self, name: &str) -> bool {
        Path::new("/sys/module").join(name).is_dir()
    }

    fn find(&self, name: &str) -> Result<PathBuf, Error> {
        let matches: Vec<&PathBuf> = self
            .files
            .iter()
            .filter(|(found, _)| found == name)
            .map(|(_, path)| path)
            .collect();
        match matches.as_slice() {
            [] => Err(Error::NotFound(name.to_owned())),
            [one] => Ok((*one).clone()),
            many => Err(Error::Ambiguous(name.to_owned(), many.len())),
        }
    }

    fn load(&mut self, path: &Path) -> Result<bool, String> {
        let file = fs::File::open(path).map_err(|err| format!("open:{err}"))?;
        // `vsock.ko` has the extension `ko`, `vsock.ko.xz` the extension `xz`.
        let compressed = path.extension().is_some_and(|ext| ext != "ko");
        let flags = if compressed {
            MODULE_INIT_COMPRESSED_FILE
        } else {
            0
        };
        // SAFETY: finit_module takes a descriptor this function holds open, a
        // NUL-terminated empty parameter string and flags.
        let rc = unsafe {
            libc::syscall(
                libc::SYS_finit_module,
                file.as_raw_fd(),
                c"".as_ptr(),
                flags,
            )
        };
        if rc == 0 {
            return Ok(true);
        }
        let err = io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::EEXIST) {
            Ok(false)
        } else {
            Err(err.to_string().replace(' ', "_"))
        }
    }

    fn lock(&mut self) -> Result<(), String> {
        fs::write(MODULES_DISABLED, b"1").map_err(|err| format!("write:{err}"))?;
        let back = fs::read_to_string(MODULES_DISABLED).map_err(|err| format!("read:{err}"))?;
        if back.trim() == "1" {
            Ok(())
        } else {
            Err(format!("reads-{}", back.trim()))
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    /// A kernel that records every step.
    #[derive(Default)]
    struct Recorder {
        steps: Vec<String>,
        builtin: Vec<&'static str>,
        absent: Vec<&'static str>,
        missing: Vec<&'static str>,
        refuse: Vec<&'static str>,
        present: Vec<&'static str>,
        lock_fails: bool,
    }

    impl Kernel for Recorder {
        fn builtin(&self, name: &str) -> bool {
            self.builtin.contains(&name)
        }

        fn present(&self, name: &str) -> bool {
            !self.absent.contains(&name)
        }

        fn find(&self, name: &str) -> Result<PathBuf, Error> {
            if self.missing.contains(&name) {
                Err(Error::NotFound(name.to_owned()))
            } else {
                Ok(PathBuf::from(format!("/m/kernel/{name}.ko.zst")))
            }
        }

        fn load(&mut self, path: &Path) -> Result<bool, String> {
            let name = module_name(&path.file_name().unwrap().to_string_lossy()).unwrap();
            self.steps.push(format!("load {name}"));
            if self.refuse.contains(&name.as_str()) {
                return Err("Exec_format_error".to_owned());
            }
            Ok(!self.present.contains(&name.as_str()))
        }

        fn lock(&mut self) -> Result<(), String> {
            self.steps.push("lock".to_owned());
            if self.lock_fails {
                Err("write:Permission denied".to_owned())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn the_door_modules_load_in_order_then_the_extras_then_the_lock() {
        let mut kernel = Recorder::default();
        let done = load_and_lock(&mut kernel, &["overlay".to_owned()]).unwrap();
        assert_eq!(
            kernel.steps,
            [
                "load vsock",
                "load vmw_vsock_virtio_transport_common",
                "load vmw_vsock_virtio_transport",
                "load overlay",
                "lock"
            ]
        );
        assert_eq!(done.len(), 4);
        assert_eq!(
            evidence(&done),
            "vsock=loaded,vmw_vsock_virtio_transport_common=loaded,\
             vmw_vsock_virtio_transport=loaded,overlay=loaded,modules_disabled=1"
        );
    }

    #[test]
    fn builtin_and_present_modules_still_end_in_the_lock() {
        let mut kernel = Recorder {
            builtin: vec!["vsock"],
            present: vec!["vmw_vsock_virtio_transport_common"],
            ..Recorder::default()
        };
        let done = load_and_lock(&mut kernel, &[]).unwrap();
        assert_eq!(
            kernel.steps,
            [
                "load vmw_vsock_virtio_transport_common",
                "load vmw_vsock_virtio_transport",
                "lock"
            ]
        );
        assert_eq!(done[0].1, Loaded::Builtin);
        assert_eq!(done[1].1, Loaded::Present);
    }

    /// A module `modules.builtin` lists but the kernel does not show fails
    /// closed: nothing loads after it, nothing is locked, the agent never
    /// runs.
    #[test]
    fn a_builtin_the_kernel_does_not_show_fails_closed() {
        let mut kernel = Recorder {
            builtin: vec!["vsock"],
            absent: vec!["vsock"],
            ..Recorder::default()
        };
        assert_eq!(
            load_and_lock(&mut kernel, &[]).unwrap_err(),
            Error::BuiltinAbsent("vsock".to_owned())
        );
        assert!(kernel.steps.is_empty());
    }

    #[test]
    fn a_failure_stops_before_anything_else_and_before_the_lock() {
        let mut kernel = Recorder {
            refuse: vec!["vmw_vsock_virtio_transport_common"],
            ..Recorder::default()
        };
        let err = load_and_lock(&mut kernel, &["overlay".to_owned()]).unwrap_err();
        assert_eq!(
            err,
            Error::Load(
                "vmw_vsock_virtio_transport_common".to_owned(),
                "Exec_format_error".to_owned()
            )
        );
        assert_eq!(
            kernel.steps,
            ["load vsock", "load vmw_vsock_virtio_transport_common"]
        );

        let mut kernel = Recorder {
            missing: vec!["vsock"],
            ..Recorder::default()
        };
        assert_eq!(
            load_and_lock(&mut kernel, &[]).unwrap_err(),
            Error::NotFound("vsock".to_owned())
        );
        assert!(kernel.steps.is_empty());

        let mut kernel = Recorder {
            lock_fails: true,
            ..Recorder::default()
        };
        assert!(matches!(
            load_and_lock(&mut kernel, &[]),
            Err(Error::Lock(_))
        ));
        assert_eq!(kernel.steps.last().map(String::as_str), Some("lock"));
    }

    #[test]
    fn names_with_a_path_are_refused_before_any_step() {
        for bad in ["", "../vsock", "a/b", ".."] {
            let mut kernel = Recorder::default();
            let err = load_and_lock(&mut kernel, &[bad.to_owned()]).unwrap_err();
            assert_eq!(err, Error::BadName(bad.to_owned()));
            assert_eq!(kernel.steps.len(), 3, "the door modules came first");
            assert!(!kernel.steps.contains(&"lock".to_owned()));
        }
    }

    #[test]
    fn module_files_are_found_compressed_or_not_and_exactly_once() {
        assert_eq!(module_name("vsock.ko"), Some("vsock".to_owned()));
        assert_eq!(module_name("vsock.ko.xz"), Some("vsock".to_owned()));
        assert_eq!(
            module_name("vmw-vsock_virtio_transport.ko.zst"),
            Some("vmw_vsock_virtio_transport".to_owned())
        );
        assert_eq!(module_name("vsock.ko.gz"), None);
        assert_eq!(module_name(".ko"), None);
        assert_eq!(module_name("modules.dep"), None);

        let dir = std::env::temp_dir().join(format!("hum465-modules-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let net = dir.join("kernel/net/vmw_vsock");
        fs::create_dir_all(&net).unwrap();
        fs::create_dir_all(dir.join("kernel/fs/fuse")).unwrap();
        fs::write(net.join("vsock.ko.zst"), b"").unwrap();
        fs::write(net.join("vmw_vsock_virtio_transport.ko"), b"").unwrap();
        fs::write(dir.join("kernel/fs/fuse/twice.ko.xz"), b"").unwrap();
        fs::write(net.join("twice.ko"), b"").unwrap();
        fs::write(dir.join("modules.builtin"), "kernel/fs/fuse/virtiofs.ko\n").unwrap();
        let kernel = Running::open(&dir).unwrap();
        assert_eq!(kernel.find("vsock").unwrap(), net.join("vsock.ko.zst"));
        assert_eq!(
            kernel.find("vmw_vsock_virtio_transport").unwrap(),
            net.join("vmw_vsock_virtio_transport.ko")
        );
        assert_eq!(
            kernel.find("twice").unwrap_err(),
            Error::Ambiguous("twice".to_owned(), 2)
        );
        assert_eq!(
            kernel.find("overlay").unwrap_err(),
            Error::NotFound("overlay".to_owned())
        );
        assert!(kernel.builtin("virtiofs"));
        assert!(!kernel.builtin("vsock"));
        fs::remove_dir_all(&dir).unwrap();

        assert!(matches!(
            Running::open(Path::new("/nonexistent-hum465")),
            Err(Error::Directory(_))
        ));
    }
}
