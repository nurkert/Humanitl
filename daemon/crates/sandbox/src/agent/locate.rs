//! Finding the agent program outside the sandbox's own mounts (HUM-278).
//!
//! The daemon usually runs under the systemd user manager, whose `PATH` is
//! `/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin`. Agents install per
//! user: the `OpenCode` installer puts a single native binary into
//! `~/.opencode/bin`, and many tools land in `~/.local/bin` or `~/bin`. None of
//! these is in the daemon's `PATH`, and none is visible in the sandbox, which
//! binds `/usr` read-only and nothing under `/home`.
//!
//! This module answers two questions, both without starting anything:
//!
//! 1. Where does the agent program live on this machine? [`search`] looks in
//!    the given directories in order ([`user_bin_dirs`] after the daemon's
//!    `PATH`) and accepts a candidate only when [`check_program`] does.
//! 2. How does it get into the sandbox? [`AgentProgram`] is one read-only bind
//!    of exactly that file at [`AGENT_PROGRAM_DIR`]`/<name>`, which the launcher
//!    adds next to the shim. The adapter cannot add mounts; this bind is the
//!    daemon's, and it is one regular file, never a directory.
//!
//! **Only native executables.** A script found this way is not bound: a
//! wrapper in `~/.local/bin` typically starts a host path the sandbox does not
//! have, and an npm entry point needs its `node_modules` next to it. Binding
//! the one file would start and then fail. Such a candidate is reported
//! ([`Skip::Script`]) so the finding can name it.
//!
//! **Only files nobody else can replace.** The file and its directory must be
//! owned by this user or by root and must not be writable by others. Group
//! write is accepted only for the user's private group (the Debian default
//! umask 002 leaves `~/.local/bin` at `drwxrwxr-x`), see [`Trust`].

use std::ffi::OsStr;
use std::fs::Metadata;
use std::io::Read as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};

use rustix::fs::{Mode, OFlags};

use crate::pinned::{PinnedSource, SourceKind};

/// The directory in the sandbox where the resolved agent program is bound.
///
/// Under `/run/humanitl`, which never comes from the host and which no
/// adapter file may target ([`super::RESERVED_FILE_TREES`]).
pub const AGENT_PROGRAM_DIR: &str = "/run/humanitl/agent";

/// The per-user directories searched after the daemon's `PATH`, relative to
/// `$HOME`, in this order.
///
/// `.opencode/bin` comes first because the `OpenCode` installer itself puts its
/// single native binary there, while `~/.local/bin` often holds a wrapper
/// script for it.
pub const USER_BIN_DIRS: &[&str] = &[".opencode/bin", ".local/bin", "bin"];

/// The ELF magic: the first four bytes of a native executable.
const ELF_MAGIC: &[u8; 4] = b"\x7fELF";

/// The per-user directories under this home, in search order.
#[must_use]
pub fn user_bin_dirs(home: &Path) -> Vec<PathBuf> {
    USER_BIN_DIRS.iter().map(|dir| home.join(dir)).collect()
}

/// Who may own and write a file that the daemon binds into the sandbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trust {
    uid: u32,
    private_gid: Option<u32>,
}

impl Trust {
    /// Trust for this user, with an explicitly named private group.
    #[must_use]
    pub const fn new(uid: u32, private_gid: Option<u32>) -> Self {
        Self { uid, private_gid }
    }

    /// Trust for the user this process runs as.
    ///
    /// The private group is the primary group when `/etc/group` names it after
    /// the user and lists no further members (the user private group scheme).
    /// A shared primary group such as `users` does not qualify, and neither
    /// does a group only known to NSS: then group write is refused.
    #[must_use]
    pub fn of_this_process() -> Self {
        let uid = rustix::process::getuid().as_raw();
        let gid = rustix::process::getgid().as_raw();
        let passwd = std::fs::read_to_string("/etc/passwd").unwrap_or_default();
        let group = std::fs::read_to_string("/etc/group").unwrap_or_default();
        Self::new(uid, private_group(&passwd, &group, uid, gid))
    }

    /// Whether a file or directory with this metadata is under the user's
    /// control alone.
    #[must_use]
    pub fn admits(&self, meta: &Metadata) -> bool {
        self.admits_raw(meta.uid(), meta.gid(), meta.mode())
    }

    const fn admits_raw(&self, owner: u32, group: u32, mode: u32) -> bool {
        let trusted_owner = owner == self.uid || owner == 0;
        let others_write = mode & 0o002 != 0;
        let group_writable = mode & 0o020 != 0;
        let private = match self.private_gid {
            Some(gid) => gid == group,
            None => false,
        };
        trusted_owner && !others_write && (!group_writable || private)
    }
}

/// The primary group, if it is the user's private group.
fn private_group(passwd: &str, group: &str, uid: u32, gid: u32) -> Option<u32> {
    let accounts: Vec<(&str, u32, u32)> = passwd
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(':');
            let name = fields.next()?;
            let account: u32 = fields.nth(1)?.parse().ok()?;
            let primary: u32 = fields.next()?.parse().ok()?;
            Some((name, account, primary))
        })
        .collect();
    let user = accounts
        .iter()
        .find(|(_, line_uid, _)| *line_uid == uid)
        .map(|(name, _, _)| *name)?;
    // Another account with this group as its primary group is a member that
    // `/etc/group` does not list.
    if accounts
        .iter()
        .any(|(_, line_uid, line_gid)| *line_gid == gid && *line_uid != uid)
    {
        return None;
    }
    group.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let line_gid: u32 = fields.nth(1)?.parse().ok()?;
        let members = fields.next().unwrap_or_default();
        (line_gid == gid && name == user && members.trim().is_empty()).then_some(gid)
    })
}

/// Why a candidate was not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skip {
    /// Not an executable regular file for this user.
    NotExecutable,
    /// The file or its directory can be replaced by someone else.
    Untrusted,
    /// A script or other non-native file; it would need files beside it.
    Script,
}

impl Skip {
    /// The reason as a phrase for a finding.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::NotExecutable => "not executable by this user",
            Self::Untrusted => "it or its directory is writable by other users",
            Self::Script => {
                "a script, which may start files outside the sandbox; point agent.command at the \
                 native binary it starts"
            }
        }
    }
}

/// The result of a search: what was found, where it looked, what it passed
/// over.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Search {
    /// The canonical path of the accepted program.
    pub found: Option<PathBuf>,
    /// The directories searched, in order.
    pub searched: Vec<PathBuf>,
    /// Candidates that exist and were not taken.
    pub skipped: Vec<(PathBuf, Skip)>,
}

impl Search {
    /// The searched directories as one line.
    #[must_use]
    pub fn searched_display(&self) -> String {
        join_paths(&self.searched)
    }

    /// The skipped candidates as one sentence, empty without any.
    #[must_use]
    pub fn skipped_display(&self) -> String {
        self.skipped
            .iter()
            .map(|(path, skip)| format!("{} was passed over: {}", path.display(), skip.describe()))
            .collect::<Vec<_>>()
            .join("; ")
    }
}

fn join_paths(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

/// Opens the agent program once and checks it through that descriptor.
///
/// The file is opened with `O_PATH | O_NOFOLLOW` after resolving symlinks
/// ([`PinnedSource`], the same path HUM-263 takes for `mounts.extra_ro`);
/// owner, mode, type and ELF header are read from the opened inode, not from
/// the path again. The launcher binds this descriptor, so a swap after the
/// check changes nothing the sandbox sees. The directory is checked too: it
/// decides whether someone else could swap the file before the next start.
///
/// # Errors
///
/// The [`Skip`] that applies; a path that does not exist is
/// [`Skip::NotExecutable`].
pub(crate) fn pin_program(path: &Path, trust: &Trust) -> Result<PinnedSource, Skip> {
    let pinned =
        PinnedSource::open(path, SourceKind::RegularFile).map_err(|_| Skip::NotExecutable)?;
    if rustix::fs::access(pinned.proc_path(), rustix::fs::Access::EXEC_OK).is_err() {
        return Err(Skip::NotExecutable);
    }
    let file = rustix::fs::fstat(pinned.fd()).map_err(|_| Skip::NotExecutable)?;
    if !trust.admits_raw(file.st_uid, file.st_gid, file.st_mode) {
        return Err(Skip::Untrusted);
    }
    let dir = pinned
        .resolved()
        .parent()
        .and_then(|parent| {
            rustix::fs::open(
                parent,
                OFlags::PATH | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .ok()
        })
        .and_then(|fd| rustix::fs::fstat(&fd).ok())
        .ok_or(Skip::Untrusted)?;
    if !trust.admits_raw(dir.st_uid, dir.st_gid, dir.st_mode) {
        return Err(Skip::Untrusted);
    }
    let mut header = Vec::with_capacity(ELF64_HEADER_LEN);
    std::fs::File::open(pinned.proc_path())
        .and_then(|file| file.take(ELF64_HEADER_LEN as u64).read_to_end(&mut header))
        .map_err(|_| Skip::NotExecutable)?;
    if !is_native_header(&header) {
        return Err(Skip::Script);
    }
    Ok(pinned)
}

/// Whether this file may be bound into the sandbox as the agent program;
/// the canonical path when it may (`pin_program`).
///
/// # Errors
///
/// The [`Skip`] that applies.
pub fn check_program(path: &Path, trust: &Trust) -> Result<PathBuf, Skip> {
    pin_program(path, trust).map(|pinned| pinned.resolved().to_path_buf())
}

/// The length of an ELF32 file header.
const ELF32_HEADER_LEN: usize = 52;
/// The length of an ELF64 file header.
const ELF64_HEADER_LEN: usize = 64;

/// Whether these bytes start a native executable: ELF magic, a known class
/// and byte order, a complete header for that class, and the type `EXEC` or
/// `DYN` (a PIE). A core dump or an object file is not a program.
fn is_native_header(bytes: &[u8]) -> bool {
    const ET_EXEC: u16 = 2;
    const ET_DYN: u16 = 3;
    if bytes.len() < ELF32_HEADER_LEN || &bytes[..4] != ELF_MAGIC {
        return false;
    }
    let needed = match bytes[4] {
        1 => ELF32_HEADER_LEN,
        2 => ELF64_HEADER_LEN,
        _ => return false,
    };
    if bytes.len() < needed {
        return false;
    }
    let e_type = match bytes[5] {
        1 => u16::from_le_bytes([bytes[16], bytes[17]]),
        2 => u16::from_be_bytes([bytes[16], bytes[17]]),
        _ => return false,
    };
    e_type == ET_EXEC || e_type == ET_DYN
}

/// Searches a bare command name in these directories, in order.
///
/// Relative and empty entries are skipped: they would resolve against the
/// daemon's working directory, which says nothing about the user's intent.
#[must_use]
pub fn search(command: &OsStr, dirs: &[PathBuf], trust: &Trust) -> Search {
    let mut result = Search::default();
    for dir in dirs.iter().filter(|dir| dir.is_absolute()) {
        result.searched.push(dir.clone());
        let candidate = dir.join(command);
        if std::fs::symlink_metadata(&candidate).is_err() {
            continue;
        }
        match check_program(&candidate, trust) {
            Ok(found) => {
                result.found = Some(found);
                return result;
            }
            Err(skip) => result.skipped.push((candidate, skip)),
        }
    }
    result
}

/// The command that points `agent.command` at an absolute path (HUM-278).
///
/// The path is a placeholder: nothing usable was found, so the user names it.
/// `AGENT_001` and `DOCTOR_007` offer the same line.
#[must_use]
pub fn agent_command_fix(command: &str) -> String {
    format!("humanitl config set agent.command '[\"/absolute/path/to/{command}\"]'")
}

/// The directories to search: the daemon's `PATH`, then the per-user ones.
#[must_use]
pub fn search_dirs(host_path: Option<&OsStr>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = host_path
        .map(|path| std::env::split_paths(path).collect())
        .unwrap_or_default();
    if let Some(home) = home.filter(|home| home.is_absolute()) {
        for dir in user_bin_dirs(home) {
            if !dirs.contains(&dir) {
                dirs.push(dir);
            }
        }
    }
    dirs
}

/// The agent program bound read-only into the sandbox.
///
/// The launcher opens `host` again and binds the descriptor
/// (`pin_program`); `host` is the canonical path, `dst` carries the name
/// the user or the adapter asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProgram {
    /// The canonical file on the host.
    pub host: PathBuf,
    /// Where it lies in the sandbox: [`AGENT_PROGRAM_DIR`]`/<file name>`.
    pub dst: PathBuf,
}

impl AgentProgram {
    /// The bind for this host file under the name that was asked for.
    #[must_use]
    pub fn new(host: PathBuf, name: &OsStr) -> Self {
        let dst = Path::new(AGENT_PROGRAM_DIR).join(name);
        Self { host, dst }
    }
}

/// The bind the agent command needs, if the sandbox cannot start it as is.
///
/// `None` when the command is already startable inside, when the answer is
/// undecidable, when it is a path relative to the project, or when no
/// acceptable file exists on the host. In all these cases the command stays
/// as it is, and the preflight reports what applies.
#[must_use]
pub fn plan_agent_program(ctx: &super::AgentContext, command: &OsStr) -> Option<AgentProgram> {
    // Only the agent the configuration names (`agent.command` or the
    // adapter's default). A command after `--` is the caller's, and binding
    // it would carry any host file past the mount denylist.
    if ctx.agent_command_from_caller {
        return None;
    }
    let named = Path::new(command);
    let name = named.file_name()?;
    let trust = ctx.trust.unwrap_or_else(Trust::of_this_process);
    if named.is_absolute() {
        if ctx.reaches_program(named) {
            return None;
        }
        return check_program(named, &trust)
            .ok()
            .map(|host| AgentProgram::new(host, name));
    }
    if named.components().count() != 1 {
        return None;
    }
    if ctx.look_up_in_sandbox_path(command) != super::SandboxLookup::Missing {
        return None;
    }
    host_search(ctx, command)
        .found
        .map(|host| AgentProgram::new(host, name))
}

/// The one search the preflight and the plan share: the daemon's `PATH`,
/// then the per-user directories, with the rules of `pin_program`. The
/// doctor runs the same [`search`] over the same [`search_dirs`].
#[must_use]
pub fn host_search(ctx: &super::AgentContext, command: &OsStr) -> Search {
    let trust = ctx.trust.unwrap_or_else(Trust::of_this_process);
    let dirs = search_dirs(ctx.host_path.as_deref(), ctx.host_home.as_deref());
    search(command, &dirs, &trust)
}

/// Plans the bind for the first word of `command` and points that word at
/// the program's place in the sandbox.
///
/// The one call the daemon and the local CLI make, so both start the same
/// file. Returns the bind for [`crate::profile::SessionContext::agent_program`];
/// without one, `command` is left untouched.
pub fn bind_agent_program(
    ctx: &super::AgentContext,
    command: &mut [std::ffi::OsString],
) -> Option<AgentProgram> {
    let first = command.first_mut()?;
    let program = plan_agent_program(ctx, first)?;
    *first = program.dst.clone().into_os_string();
    Some(program)
}

/// The bind of the configured agent for a session whose command came from
/// the caller (HUM-280).
///
/// `humanitl run -- bash` opens a shell, and the person starts `opencode`
/// or `pi` there. The caller's own command is never bound
/// ([`plan_agent_program`] refuses it); the agent the configuration names
/// (`configured`, from `agent.command`, otherwise the adapter's default) is
/// planned by the same rules as for a daemon-started agent and lies at
/// [`AGENT_PROGRAM_DIR`]`/<name>`, which the sandbox puts first on `PATH`
/// whenever a bind exists. `ctx` is the caller's context; everything but the
/// command is the same for both.
#[must_use]
pub fn plan_configured_agent(
    ctx: &super::AgentContext,
    adapter: &dyn super::AgentAdapter,
    configured: Option<Vec<std::ffi::OsString>>,
) -> Option<AgentProgram> {
    let agent = ctx.clone().with_command_override(configured);
    let command = adapter.command(&agent);
    plan_agent_program(&agent, command.first()?)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::fs::PermissionsExt as _;

    use super::*;

    fn uid() -> u32 {
        rustix::process::getuid().as_raw()
    }

    /// A copy of a real native executable, so the ELF check sees ELF.
    fn native(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        std::fs::copy("/bin/true", &path)
            .or_else(|_| std::fs::copy("/usr/bin/true", &path))
            .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn home_with_local_bin() -> (tempfile::TempDir, PathBuf) {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join(".local/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        (home, bin)
    }

    #[test]
    fn an_agent_in_local_bin_is_found_while_path_lacks_it() {
        let (home, bin) = home_with_local_bin();
        let program = native(&bin, "opencode");
        let empty = tempfile::tempdir().unwrap();
        let path = empty.path().as_os_str().to_owned();
        let dirs = search_dirs(Some(&path), Some(home.path()));

        let found = search(OsStr::new("opencode"), &dirs, &Trust::new(uid(), None));

        assert_eq!(found.found, Some(std::fs::canonicalize(program).unwrap()));
        assert!(found.searched.contains(&empty.path().to_path_buf()));
        assert!(found.searched.contains(&bin));
    }

    #[test]
    fn a_group_writable_dir_is_skipped_unless_the_group_is_private() {
        let (home, bin) = home_with_local_bin();
        native(&bin, "opencode");
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o775)).unwrap();
        let dirs = user_bin_dirs(home.path());

        let shared = search(OsStr::new("opencode"), &dirs, &Trust::new(uid(), None));
        assert_eq!(shared.found, None);
        assert_eq!(
            shared.skipped,
            vec![(bin.join("opencode"), Skip::Untrusted)]
        );

        let gid = std::fs::metadata(&bin).unwrap().gid();
        let private = search(OsStr::new("opencode"), &dirs, &Trust::new(uid(), Some(gid)));
        assert!(private.found.is_some(), "{private:?}");
    }

    #[test]
    fn a_world_writable_file_and_a_foreign_owner_are_refused() {
        assert!(!Trust::new(1000, None).admits_raw(1000, 1000, 0o100_757));
        assert!(!Trust::new(1000, None).admits_raw(1001, 1001, 0o100_755));
        assert!(Trust::new(1000, None).admits_raw(0, 0, 0o100_755));
        assert!(Trust::new(1000, None).admits_raw(1000, 1000, 0o100_755));
    }

    #[test]
    fn a_script_is_passed_over_for_a_native_binary_later_in_the_order() {
        let home = tempfile::tempdir().unwrap();
        let opencode_bin = home.path().join(".opencode/bin");
        let local_bin = home.path().join(".local/bin");
        for dir in [&opencode_bin, &local_bin] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        // Only the script exists: it is reported, not taken.
        let script = local_bin.join("opencode");
        std::fs::write(
            &script,
            "#!/bin/sh\nexec \"$HOME/.opencode/bin/opencode\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dirs = user_bin_dirs(home.path());
        let trust = Trust::new(uid(), None);
        let only_script = search(OsStr::new("opencode"), &dirs, &trust);
        assert_eq!(only_script.found, None);
        assert_eq!(only_script.skipped, vec![(script, Skip::Script)]);

        let program = native(&opencode_bin, "opencode");
        let both = search(OsStr::new("opencode"), &dirs, &trust);
        assert_eq!(both.found, Some(std::fs::canonicalize(program).unwrap()));
    }

    #[test]
    fn the_private_group_needs_the_users_name_and_no_members() {
        let passwd = "root:x:0:0::/root:/bin/sh\nniko:x:1000:1000::/home/niko:/bin/sh\n";
        assert_eq!(
            private_group(passwd, "niko:x:1000:\n", 1000, 1000),
            Some(1000)
        );
        assert_eq!(private_group(passwd, "users:x:1000:\n", 1000, 1000), None);
        assert_eq!(private_group(passwd, "niko:x:1000:eve\n", 1000, 1000), None);
        assert_eq!(private_group(passwd, "niko:x:1000:\n", 1000, 100), None);
        // Another account with gid 1000 as its primary group is a member
        // `/etc/group` does not list.
        let shared = format!("{passwd}eve:x:1001:1000::/home/eve:/bin/sh\n");
        assert_eq!(private_group(&shared, "niko:x:1000:\n", 1000, 1000), None);
        // An account only NSS knows: exclusivity cannot be shown.
        assert_eq!(private_group(passwd, "niko:x:1000:\n", 1002, 1000), None);
    }

    #[test]
    fn the_bind_lands_under_the_agent_dir_with_the_file_name() {
        let program = AgentProgram::new(
            PathBuf::from("/home/u/.opencode/bin/opencode"),
            OsStr::new("opencode"),
        );
        assert_eq!(program.dst, Path::new("/run/humanitl/agent/opencode"));
    }

    /// The descriptor checked is the one bound: a swap of the path after the
    /// check does not change what the sandbox gets (HUM-278).
    #[test]
    fn the_pinned_program_survives_a_swap_of_its_path() {
        let (_home, bin) = home_with_local_bin();
        let program = native(&bin, "opencode");
        let original = std::fs::read(&program).unwrap();

        let pinned = pin_program(&program, &Trust::new(uid(), None)).expect("bindable");
        std::fs::rename(&program, bin.join("opencode.old")).unwrap();
        std::fs::write(&program, b"#!/bin/sh\necho swapped\n").unwrap();

        assert_eq!(std::fs::read(pinned.proc_path()).unwrap(), original);
    }

    #[test]
    fn only_a_complete_elf_executable_header_is_native() {
        let mut header = vec![0_u8; 64];
        header[..4].copy_from_slice(b"\x7fELF");
        header[4] = 2; // ELF64
        header[5] = 1; // little endian
        header[16] = 3; // ET_DYN
        assert!(is_native_header(&header));
        header[16] = 2; // ET_EXEC
        assert!(is_native_header(&header));
        header[16] = 4; // ET_CORE
        assert!(!is_native_header(&header));
        header[16] = 1; // ET_REL
        assert!(!is_native_header(&header));
        header[16] = 3;
        assert!(
            !is_native_header(&header[..60]),
            "an ELF64 header needs 64 bytes"
        );
        header[4] = 9;
        assert!(!is_native_header(&header), "unknown class");
        assert!(!is_native_header(b"\x7fELF"));
    }

    /// A command the caller names after `--` is never bound, only the agent
    /// the configuration names (HUM-278).
    #[test]
    fn a_callers_command_is_never_bound() {
        let (home, bin) = home_with_local_bin();
        native(&bin, "opencode");
        let ctx = super::super::AgentContext::new(
            humanitl_core::SessionId::nil(),
            PathBuf::from("/home/u/proj"),
            humanitl_config::LlmConfig::default(),
        )
        .with_host_home(Some(home.path().to_path_buf()))
        .with_trust(Trust::new(uid(), None))
        .with_sandbox_view(super::super::SandboxView::same_path(
            vec![PathBuf::from("/usr")],
            Vec::new(),
        ))
        .with_sandbox_path(Some(std::ffi::OsString::from("/usr/bin")));
        assert!(plan_agent_program(&ctx, OsStr::new("opencode")).is_some());

        let caller = ctx.with_command_from_caller(vec![std::ffi::OsString::from("opencode")]);
        assert_eq!(plan_agent_program(&caller, OsStr::new("opencode")), None);
    }
}
