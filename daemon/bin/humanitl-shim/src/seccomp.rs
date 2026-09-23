//! The seccomp filter: guarantee three, "the kernel opens no new door"
//! (`docs/SECURITY.md`, Satz 3; CONVENTIONS.md 4.8 and 4.10).
//!
//! Everything the kernel refuses to the agent is decided in this file, as data
//! first and as BPF second. [`Policy::rules`] is the table `docs/SECURITY.md`
//! cites; [`Policy::program`] renders exactly that table. The test
//! `rule_table_lists_every_rule` prints every row so a reviewer can diff the
//! document against the code.
//!
//! Shape of the program, in the order the kernel runs it:
//!
//! 1. A hand-written prelude. An architecture other than the one this binary
//!    was built for kills the process. A syscall number with the x32 bit
//!    ([`X32_SYSCALL_BIT`]) gets `EPERM`: the x32 ABI shares the `arch` value
//!    of `x86_64` and is the classic way around a filter that keys on numbers.
//!    `socket(2)` passes only when the family (arg0, low 32 bits, which is
//!    what the kernel reads for an `int`) is in `allow_families` and the type
//!    (arg1 masked with [`SOCK_TYPE_MASK`], so `SOCK_NONBLOCK|SOCK_CLOEXEC`
//!    pass) is in `allow_types`; otherwise `EPERM`. A `socket(2)` that passes
//!    the gate falls through into part 2, so a profile that denies `socket`
//!    by name still wins.
//! 2. The seccompiler program: its own architecture check (kill), then
//!    `EPERM` for every name in `deny_syscalls`, and `Allow` for everything
//!    else.
//!
//! Why the socket gate is hand-written and not a seccompiler rule: seccompiler
//! has one match action per filter and no masked-not-equal comparison, so
//! "type not in the list" cannot be expressed as a deny rule there. A dozen
//! instructions here, checked by the interpreter test below and by the forked
//! children in the same test module, are easier to audit than a few hundred
//! generated rules enumerating the complement.
//!
//! `socketpair(2)` has no rule on purpose (CONVENTIONS.md 4.11): it knows only
//! `AF_UNIX`, connects two descriptors of the same process tree and is no
//! egress. Node and Bun need it for child-process IPC.
//!
//! Two policies leave this file: the agent's, built from the environment the
//! launcher sets, and the bridge's ([`Policy::for_bridge`]), which is the
//! agent's plus `AF_UNIX`, because `connect(2)` to the proxy socket needs
//! `socket(AF_UNIX, SOCK_STREAM)`. The bridge is the only process in the
//! sandbox that may open a Unix socket, and it knows exactly one target.
//!
//! Two gates leave this file as well ([`Gate`], HUM-138). The bridge's filter
//! answers a refused `socket(2)` with `EPERM` at once ([`Gate::Silent`]). The
//! agent's filter hands the same call to the shim's parent first
//! (`SECCOMP_RET_USER_NOTIF`, [`Gate::Reported`]): the kernel parks the call,
//! the parent counts it and answers `EPERM` (`refusals.rs`). The verdict is the
//! same `EPERM` either way; what changes is that somebody learns of the
//! attempt. The parent never answers anything else, and without a listener the
//! kernel answers `ENOSYS` itself, so the gate stays closed whatever happens to
//! the parent. Everything outside the socket gate (x32, the floor, the
//! architecture) is answered by the kernel alone, as before.

use std::collections::BTreeMap;
use std::ffi::{c_long, c_uint};
use std::fmt;
use std::io;
use std::os::fd::{FromRawFd, OwnedFd};

use seccompiler::{BpfProgram, SeccompAction, SeccompFilter, SeccompRule, TargetArch, sock_filter};

/// The x32 ABI marks its syscall numbers with this bit (`__X32_SYSCALL_BIT`).
pub const X32_SYSCALL_BIT: u32 = 0x4000_0000;

/// What of `socket(2)`'s second argument is the type; the rest are flags
/// (`SOCK_NONBLOCK`, `SOCK_CLOEXEC`) that the filter lets through.
pub const SOCK_TYPE_MASK: u32 = 0xff;

/// Syscalls the filter refuses in every profile, even when the launcher
/// forgets them: the floor from CONVENTIONS.md 4.8, in the profile's order.
///
/// Die ersten zehn Namen halten fremde Prozesse, den Schlüsselbund und die
/// Ein- und Ausgabe an seccomp vorbei fern. `pidfd_getfd` gehört dazu
/// (HUM-203): wie `process_vm_*` prüft es nur `ptrace_may_access`, und ohne
/// den Eintrag hinge es allein daran, dass kein Prozess der Sandbox dumpable
/// ist. `pidfd_open` steht bewusst nur in [`SYSCALLS`] und nicht hier:
/// asyncio, Go und libuv tasten es ab, und ohne `pidfd_getfd` öffnet ein
/// Prozess-Deskriptor nichts, was nicht auch ein `kill` erreicht. Die acht danach sind die
/// Standard-Härtung aus der Tabelle in `backlog/sprint-1.md` (HUM-012), die
/// dasselbe verbietet wie das Docker-Standardprofil: einen neuen Kern laden
/// (`kexec_*`), Kernmodule tauschen (`*_module`), BPF-Programme laden
/// (`bpf`), fremde Ereigniszähler öffnen (`perf_event_open`) und
/// Seitenfehler im eigenen Adressraum selbst bedienen (`userfaultfd`, der
/// klassische Hebel, um ein Zeitfenster zwischen Prüfung und Benutzung im
/// Kern aufzuhalten). Sie stehen im Boden und nicht nur in [`SYSCALLS`],
/// damit ein Profil, das sie vergisst, sie trotzdem verbietet.
pub const FLOOR: &[&str] = &[
    "ptrace",
    "io_uring_setup",
    "io_uring_enter",
    "io_uring_register",
    "process_vm_readv",
    "process_vm_writev",
    "pidfd_getfd",
    "keyctl",
    "add_key",
    "request_key",
    "kexec_load",
    "kexec_file_load",
    "init_module",
    "finit_module",
    "delete_module",
    "bpf",
    "perf_event_open",
    "userfaultfd",
];

/// Families the launcher may name in `HUMANITL_SECCOMP_FAMILIES`.
///
/// Mirrors `humanitl_sandbox::SocketFamily`; a name missing here is a setup
/// error, never silently ignored.
pub const FAMILIES: &[Family] = &[
    Family {
        name: "AF_UNIX",
        number: libc::AF_UNIX.unsigned_abs(),
    },
    Family {
        name: "AF_INET",
        number: libc::AF_INET.unsigned_abs(),
    },
    Family {
        name: "AF_INET6",
        number: libc::AF_INET6.unsigned_abs(),
    },
];

/// Socket types the launcher may name in `HUMANITL_SECCOMP_TYPES`.
///
/// Mirrors `humanitl_sandbox::SocketType`.
pub const SOCK_TYPES: &[SockType] = &[
    SockType {
        name: "SOCK_STREAM",
        number: libc::SOCK_STREAM.unsigned_abs(),
    },
    SockType {
        name: "SOCK_DGRAM",
        number: libc::SOCK_DGRAM.unsigned_abs(),
    },
];

/// Syscalls a profile may deny by name.
///
/// Der Boden aus [`FLOOR`] zuerst, in derselben Reihenfolge, danach das, was
/// ein Profil sinnvoll ergänzen kann. A name the launcher passes that is not
/// here ends the shim with exit 126: an unknown name silently dropped would be
/// a hole nobody sees.
pub const SYSCALLS: &[Syscall] = &[
    Syscall::new("ptrace", libc::SYS_ptrace),
    Syscall::new("io_uring_setup", libc::SYS_io_uring_setup),
    Syscall::new("io_uring_enter", libc::SYS_io_uring_enter),
    Syscall::new("io_uring_register", libc::SYS_io_uring_register),
    Syscall::new("process_vm_readv", libc::SYS_process_vm_readv),
    Syscall::new("process_vm_writev", libc::SYS_process_vm_writev),
    Syscall::new("pidfd_getfd", libc::SYS_pidfd_getfd),
    Syscall::new("keyctl", libc::SYS_keyctl),
    Syscall::new("add_key", libc::SYS_add_key),
    Syscall::new("request_key", libc::SYS_request_key),
    Syscall::new("kexec_load", libc::SYS_kexec_load),
    Syscall::new("kexec_file_load", libc::SYS_kexec_file_load),
    Syscall::new("init_module", libc::SYS_init_module),
    Syscall::new("finit_module", libc::SYS_finit_module),
    Syscall::new("delete_module", libc::SYS_delete_module),
    Syscall::new("bpf", libc::SYS_bpf),
    Syscall::new("perf_event_open", libc::SYS_perf_event_open),
    Syscall::new("userfaultfd", libc::SYS_userfaultfd),
    Syscall::new("mount", libc::SYS_mount),
    Syscall::new("umount2", libc::SYS_umount2),
    Syscall::new("pivot_root", libc::SYS_pivot_root),
    Syscall::new("chroot", libc::SYS_chroot),
    Syscall::new("setns", libc::SYS_setns),
    Syscall::new("unshare", libc::SYS_unshare),
    Syscall::new("reboot", libc::SYS_reboot),
    Syscall::new("swapon", libc::SYS_swapon),
    Syscall::new("swapoff", libc::SYS_swapoff),
    Syscall::new("acct", libc::SYS_acct),
    Syscall::new("settimeofday", libc::SYS_settimeofday),
    Syscall::new("clock_settime", libc::SYS_clock_settime),
    Syscall::new("clock_adjtime", libc::SYS_clock_adjtime),
    Syscall::new("adjtimex", libc::SYS_adjtimex),
    Syscall::new("open_by_handle_at", libc::SYS_open_by_handle_at),
    Syscall::new("name_to_handle_at", libc::SYS_name_to_handle_at),
    Syscall::new("personality", libc::SYS_personality),
    Syscall::new("kcmp", libc::SYS_kcmp),
    Syscall::new("quotactl", libc::SYS_quotactl),
    Syscall::new("mbind", libc::SYS_mbind),
    Syscall::new("set_mempolicy", libc::SYS_set_mempolicy),
    Syscall::new("get_mempolicy", libc::SYS_get_mempolicy),
    Syscall::new("migrate_pages", libc::SYS_migrate_pages),
    Syscall::new("move_pages", libc::SYS_move_pages),
    Syscall::new("fsopen", libc::SYS_fsopen),
    Syscall::new("fsconfig", libc::SYS_fsconfig),
    Syscall::new("fsmount", libc::SYS_fsmount),
    Syscall::new("fspick", libc::SYS_fspick),
    Syscall::new("move_mount", libc::SYS_move_mount),
    Syscall::new("open_tree", libc::SYS_open_tree),
    Syscall::new("mount_setattr", libc::SYS_mount_setattr),
    Syscall::new("vhangup", libc::SYS_vhangup),
    Syscall::new("pidfd_open", libc::SYS_pidfd_open),
    Syscall::new("socket", libc::SYS_socket),
];

#[cfg(target_arch = "x86_64")]
const TARGET_ARCH: TargetArch = TargetArch::x86_64;
/// `AUDIT_ARCH_X86_64` = `EM_X86_64 | __AUDIT_ARCH_64BIT | __AUDIT_ARCH_LE`.
#[cfg(target_arch = "x86_64")]
const AUDIT_ARCH: u32 = 0x3e | 0x8000_0000 | 0x4000_0000;
#[cfg(target_arch = "aarch64")]
const TARGET_ARCH: TargetArch = TargetArch::aarch64;
/// `AUDIT_ARCH_AARCH64` = `EM_AARCH64 | __AUDIT_ARCH_64BIT | __AUDIT_ARCH_LE`.
#[cfg(target_arch = "aarch64")]
const AUDIT_ARCH: u32 = 0xb7 | 0x8000_0000 | 0x4000_0000;
#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("humanitl-shim supports x86_64 and aarch64 only (seccomp.rs)");

// Classic BPF opcodes and the layout of `struct seccomp_data`, from
// <linux/bpf_common.h> and <linux/seccomp.h>. Spelled out rather than cast
// from `libc` so the prelude reads like the kernel documentation; the test
// `opcodes_match_libc` pins them to libc's values.
const BPF_LD_W_ABS: u16 = 0x20;
const BPF_ALU_AND_K: u16 = 0x54;
const BPF_JMP_JEQ_K: u16 = 0x15;
const BPF_JMP_JSET_K: u16 = 0x45;
const BPF_RET_K: u16 = 0x06;
const DATA_NR: u32 = 0;
const DATA_ARCH: u32 = 4;
const DATA_ARG0_LOW: u32 = 16;
const DATA_ARG1_LOW: u32 = 24;
/// `SECCOMP_FILTER_FLAG_NEW_LISTENER` as the prelude compares it: the low word
/// of arg1 of `seccomp(2)`.
const NEW_LISTENER: u32 = 1 << 3;
/// Instructions of the prelude that do not depend on the number of families
/// and types.
const PRELUDE_FIXED: usize = 17;
const RET_KILL_PROCESS: u32 = libc::SECCOMP_RET_KILL_PROCESS;
const RET_EPERM: u32 =
    libc::SECCOMP_RET_ERRNO | (libc::EPERM.unsigned_abs() & libc::SECCOMP_RET_DATA);
/// The kernel parks the call and asks the listener (`refusals.rs`).
const RET_USER_NOTIF: u32 = libc::SECCOMP_RET_USER_NOTIF;

/// How the socket gate answers a call it refuses (HUM-138).
///
/// Both answers end in `EPERM` for the caller. They differ in who knows: with
/// [`Gate::Silent`] only the caller does, with [`Gate::Reported`] the shim's
/// parent counts the attempt before it answers, and the host shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// `SECCOMP_RET_ERRNO | EPERM` straight from the kernel. The bridge's
    /// filter, and the agent's when the kernel refuses a listener.
    Silent,
    /// `SECCOMP_RET_USER_NOTIF`: the listener answers `EPERM` and counts. The
    /// agent's filter.
    Reported,
}

impl Gate {
    const fn refuse(self) -> u32 {
        match self {
            Self::Silent => RET_EPERM,
            Self::Reported => RET_USER_NOTIF,
        }
    }

    const fn verdict(self) -> Verdict {
        match self {
            Self::Silent => Verdict::Eperm,
            Self::Reported => Verdict::ReportedEperm,
        }
    }
}

/// A socket family by the name the profile uses and the number `socket(2)`
/// receives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Family {
    /// The name in the profile (`AF_INET`).
    pub name: &'static str,
    /// The value of arg0.
    pub number: u32,
}

/// A socket type by the name the profile uses and the number `socket(2)`
/// receives in the low byte of arg1.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SockType {
    /// The name in the profile (`SOCK_STREAM`).
    pub name: &'static str,
    /// The value of `arg1 & 0xff`.
    pub number: u32,
}

/// A syscall by name and number for the architecture this binary runs on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Syscall {
    /// The name as in `deny_syscalls`.
    pub name: &'static str,
    /// The number the kernel sees.
    pub nr: c_long,
}

impl Syscall {
    const fn new(name: &'static str, nr: c_long) -> Self {
        Self { name, nr }
    }
}

/// What the filter refuses, as data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Policy {
    families: Vec<Family>,
    types: Vec<SockType>,
    deny: Vec<Syscall>,
}

/// One row of the rule table: which syscall, under which condition, gets
/// which verdict, and which part of the program carries it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rule {
    /// The syscall(s) the row is about.
    pub subject: Subject,
    /// When the row applies.
    pub condition: Condition,
    /// What the kernel answers.
    pub verdict: Verdict,
    /// Hand-written prelude or seccompiler program.
    pub origin: Origin,
}

/// The syscall(s) a rule is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Subject {
    /// Every syscall.
    Every,
    /// `socket(2)`.
    Socket,
    /// `seccomp(2)` (HUM-138).
    Seccomp,
    /// One named syscall.
    Named(Syscall),
}

/// When a rule applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    /// `seccomp_data.arch` is not the architecture this binary was built for.
    ArchMismatch,
    /// `seccomp_data.nr & 0x40000000` is set.
    X32Bit,
    /// arg1 (low 32 bits) carries `SECCOMP_FILTER_FLAG_NEW_LISTENER`.
    ListenerFlag,
    /// arg0 (low 32 bits) is none of these families.
    FamilyNotIn(Vec<Family>),
    /// `arg1 & 0xff` is none of these types.
    TypeNotIn(Vec<SockType>),
    /// Unconditionally.
    Always,
}

/// What the kernel answers when a rule applies.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// `SECCOMP_RET_KILL_PROCESS`.
    KillProcess,
    /// `SECCOMP_RET_ERRNO | EPERM`.
    Eperm,
    /// `SECCOMP_RET_USER_NOTIF`, which the shim's parent answers with `EPERM`
    /// after counting the attempt ([`Gate::Reported`]).
    ReportedEperm,
}

/// Which part of the program carries a rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin {
    /// The hand-written prelude in this file.
    Prelude,
    /// The program seccompiler generates from the deny map.
    Seccompiler,
}

/// Why a policy could not be built or applied.
#[derive(Debug)]
pub enum Error {
    /// A family name the table does not know.
    UnknownFamily(String),
    /// A socket type name the table does not know.
    UnknownType(String),
    /// A syscall name the table does not know.
    UnknownSyscall(String),
    /// A list the launcher set is empty; absent means the default, empty
    /// means a mistake.
    EmptyList(&'static str),
    /// More families or types than one BPF jump can skip.
    TooManyEntries,
    /// seccompiler refused the deny map.
    Build(seccompiler::Error),
    /// `prctl(PR_SET_NO_NEW_PRIVS)` failed.
    NoNewPrivs(io::Error),
    /// `seccomp(2)` refused the program.
    Apply(seccompiler::Error),
    /// `seccomp(2)` refused the program with a listener
    /// (`SECCOMP_FILTER_FLAG_NEW_LISTENER`); the errno says why, `EBUSY`
    /// when a filter further up already has one.
    Listener(io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownFamily(name) => write!(f, "unknown socket family {name:?}"),
            Self::UnknownType(name) => write!(f, "unknown socket type {name:?}"),
            Self::UnknownSyscall(name) => write!(f, "unknown syscall name {name:?}"),
            Self::EmptyList(var) => write!(f, "{var} is set but empty"),
            Self::TooManyEntries => write!(f, "too many families or types for one BPF jump"),
            Self::Build(err) => write!(f, "cannot build the filter: {err}"),
            Self::NoNewPrivs(err) => write!(f, "PR_SET_NO_NEW_PRIVS failed: {err}"),
            Self::Apply(err) => write!(f, "seccomp(2) refused the filter: {err}"),
            Self::Listener(err) => {
                write!(f, "seccomp(2) refused the filter with a listener: {err}")
            }
        }
    }
}

impl std::error::Error for Error {}

impl Policy {
    /// The policy the launcher describes in the environment.
    ///
    /// `families` is `HUMANITL_SECCOMP_FAMILIES` (absent: `AF_INET,AF_INET6`),
    /// `types` is `HUMANITL_SECCOMP_TYPES` (absent: `SOCK_STREAM`), `deny` is
    /// `HUMANITL_SECCOMP_DENY` (absent: the floor). The deny list is always
    /// joined with [`FLOOR`], floor first, without duplicates; a name that is
    /// not in [`SYSCALLS`] is an error, not a warning.
    pub fn from_env(
        families: Option<&str>,
        types: Option<&str>,
        deny: Option<&str>,
    ) -> Result<Self, Error> {
        let families = match families {
            None => vec![family("AF_INET")?, family("AF_INET6")?],
            Some(text) => parse_list(text, "HUMANITL_SECCOMP_FAMILIES", family)?,
        };
        let types = match types {
            None => vec![sock_type("SOCK_STREAM")?],
            Some(text) => parse_list(text, "HUMANITL_SECCOMP_TYPES", sock_type)?,
        };
        let mut list = Vec::new();
        for name in FLOOR {
            push_unique(&mut list, syscall(name)?);
        }
        if let Some(text) = deny {
            for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                push_unique(&mut list, syscall(item)?);
            }
        }
        Ok(Self {
            families,
            types,
            deny: list,
        })
    }

    /// The bridge's policy: this one plus `AF_UNIX`.
    ///
    /// The parent shim holds the bridge and must `connect(2)` to the proxy
    /// socket for every accepted connection, which needs
    /// `socket(AF_UNIX, SOCK_STREAM)`. Everything else stays as strict as for
    /// the agent, so every process in the sandbox carries a filter, PID 1
    /// included: since HUM-203 the parent shim is PID 1 (`--as-pid-1`), and
    /// ESC-1 `seccomp_every_process` no longer makes an exception for it.
    #[must_use]
    pub fn for_bridge(&self) -> Self {
        let mut bridge = self.clone();
        if let Ok(unix) = family("AF_UNIX") {
            push_unique(&mut bridge.families, unix);
        }
        bridge
    }

    /// The families `socket(2)` may use.
    #[must_use]
    pub fn families(&self) -> &[Family] {
        &self.families
    }

    /// The types `socket(2)` may use.
    #[must_use]
    pub fn types(&self) -> &[SockType] {
        &self.types
    }

    /// The rule table for the socket gate `gate`, in the order the kernel
    /// evaluates it.
    ///
    /// This is what `docs/SECURITY.md` cites. [`Policy::program`] renders it
    /// and nothing else.
    #[must_use]
    pub fn rules(&self, gate: Gate) -> Vec<Rule> {
        let mut rules = vec![
            Rule {
                subject: Subject::Every,
                condition: Condition::ArchMismatch,
                verdict: Verdict::KillProcess,
                origin: Origin::Prelude,
            },
            Rule {
                subject: Subject::Every,
                condition: Condition::X32Bit,
                verdict: Verdict::Eperm,
                origin: Origin::Prelude,
            },
            Rule {
                subject: Subject::Seccomp,
                condition: Condition::ListenerFlag,
                verdict: Verdict::Eperm,
                origin: Origin::Prelude,
            },
            Rule {
                subject: Subject::Socket,
                condition: Condition::FamilyNotIn(self.families.clone()),
                verdict: gate.verdict(),
                origin: Origin::Prelude,
            },
            Rule {
                subject: Subject::Socket,
                condition: Condition::TypeNotIn(self.types.clone()),
                verdict: gate.verdict(),
                origin: Origin::Prelude,
            },
        ];
        rules.extend(self.deny.iter().map(|syscall| Rule {
            subject: Subject::Named(*syscall),
            condition: Condition::Always,
            verdict: Verdict::Eperm,
            origin: Origin::Seccompiler,
        }));
        rules
    }

    /// The BPF program for the socket gate `gate`: the prelude, then the
    /// seccompiler program.
    pub fn program(&self, gate: Gate) -> Result<BpfProgram, Error> {
        let mut program = prelude(&self.families, &self.types, gate)?;
        let mut map: BTreeMap<i64, Vec<SeccompRule>> = BTreeMap::new();
        for syscall in &self.deny {
            // An empty rule vector is seccompiler's "always": the syscall gets
            // the match action unconditionally.
            map.insert(syscall.nr, Vec::new());
        }
        let filter = SeccompFilter::new(
            map,
            SeccompAction::Allow,
            SeccompAction::Errno(libc::EPERM.unsigned_abs()),
            TARGET_ARCH,
        )
        .map_err(|err| Error::Build(seccompiler::Error::Backend(err)))?;
        let generated: BpfProgram = filter
            .try_into()
            .map_err(|err| Error::Build(seccompiler::Error::Backend(err)))?;
        program.extend(generated);
        Ok(program)
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let subject = match &self.subject {
            Subject::Every => "every syscall".to_owned(),
            Subject::Socket => "socket".to_owned(),
            Subject::Seccomp => "seccomp".to_owned(),
            Subject::Named(syscall) => syscall.name.to_owned(),
        };
        let condition = match &self.condition {
            Condition::ArchMismatch => format!("arch is not {}", std::env::consts::ARCH),
            Condition::X32Bit => format!("nr has the x32 bit {X32_SYSCALL_BIT:#x}"),
            Condition::ListenerFlag => format!("flags (arg1) have NEW_LISTENER {NEW_LISTENER:#x}"),
            Condition::FamilyNotIn(families) => format!(
                "family (arg0) not in {{{}}}",
                families
                    .iter()
                    .map(|family| family.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Condition::TypeNotIn(types) => format!(
                "type (arg1 & {SOCK_TYPE_MASK:#x}) not in {{{}}}",
                types
                    .iter()
                    .map(|sock_type| sock_type.name)
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Condition::Always => "always".to_owned(),
        };
        let verdict = match self.verdict {
            Verdict::KillProcess => "kill process",
            Verdict::Eperm => "EPERM",
            Verdict::ReportedEperm => "notify:EPERM",
        };
        let origin = match self.origin {
            Origin::Prelude => "prelude",
            Origin::Seccompiler => "seccompiler",
        };
        write!(
            f,
            "{subject:<18} | {condition:<46} | {verdict:<12} | {origin}"
        )
    }
}

/// Installs `program` for the calling process and all its threads.
///
/// Sets `PR_SET_NO_NEW_PRIVS` first: without it an unprivileged process may
/// not install a filter at all, and with it no later `exec` can gain
/// privileges (setuid) to shed the filter. Then `seccomp(2)` with
/// `SECCOMP_FILTER_FLAG_TSYNC`. The filter is inherited by every descendant
/// and cannot be removed.
pub fn apply(program: &BpfProgram) -> Result<(), Error> {
    // SAFETY: prctl with these constant arguments touches nothing but the
    // calling process's flags; the trailing zeros are the unused arguments
    // the man page asks for.
    let rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if rc != 0 {
        return Err(Error::NoNewPrivs(io::Error::last_os_error()));
    }
    seccompiler::apply_filter_all_threads(program).map_err(Error::Apply)
}

/// Installs `program` for the calling process with a listener and returns
/// the listener (`SECCOMP_FILTER_FLAG_NEW_LISTENER`, HUM-138).
///
/// Sets `PR_SET_NO_NEW_PRIVS` first, like [`apply`]. Without `TSYNC`: the
/// kernel refuses `TSYNC` together with a listener before Linux 5.7
/// (`SECCOMP_FILTER_FLAG_TSYNC_ESRCH`), and the only caller is the child right
/// after `fork(2)`, which has exactly one thread. Every thread and process the
/// agent starts later inherits the filter either way.
///
/// Whoever holds the returned descriptor answers every call the program parks
/// with `SECCOMP_RET_USER_NOTIF`; once nobody holds it, the kernel answers
/// those calls with `ENOSYS`. It is created with `O_CLOEXEC`, and it must never
/// reach the agent: a listener may answer "go ahead"
/// (`SECCOMP_USER_NOTIF_FLAG_CONTINUE`), and the agent would answer itself.
///
/// Allocates nothing, so a freshly forked child may call it.
pub fn apply_reporting(program: &BpfProgram) -> Result<OwnedFd, Error> {
    // SAFETY: as in `apply`: constant arguments, the calling process's flags.
    let rc = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if rc != 0 {
        return Err(Error::NoNewPrivs(io::Error::last_os_error()));
    }
    let len = u16::try_from(program.len()).map_err(|_| Error::TooManyEntries)?;
    let fprog = libc::sock_fprog {
        len,
        // seccompiler's `sock_filter` is the kernel's layout, field for
        // field; the kernel only reads through the pointer.
        filter: program.as_ptr().cast_mut().cast::<libc::sock_filter>(),
    };
    // SAFETY: `fprog` points at `program`, which outlives the call; the kernel
    // copies the instructions before it returns.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_seccomp,
            libc::c_ulong::from(libc::SECCOMP_SET_MODE_FILTER),
            libc::SECCOMP_FILTER_FLAG_NEW_LISTENER,
            &raw const fprog,
        )
    };
    if rc < 0 {
        return Err(Error::Listener(io::Error::last_os_error()));
    }
    let fd = libc::c_int::try_from(rc)
        .map_err(|_| Error::Listener(io::Error::from_raw_os_error(libc::EBADF)))?;
    // SAFETY: the kernel just created this descriptor for the caller.
    Ok(unsafe { OwnedFd::from_raw_fd(fd) })
}

/// `socket(family, type, 0)` as a probe: `Ok` when the kernel handed out a
/// descriptor (closed again at once), otherwise the errno.
///
/// Goes through `syscall(2)` so the arguments reach the kernel as full
/// 64-bit words; tests use that to show the filter reads the low 32 bits.
pub fn probe_socket(family: u64, sock_type: u64) -> Result<(), i32> {
    probe_socket_nr(libc::SYS_socket, family, sock_type)
}

/// [`probe_socket`] under the x32 number of `socket(2)`; must be refused.
pub fn probe_socket_x32(family: u64, sock_type: u64) -> Result<(), i32> {
    probe_socket_nr(
        libc::SYS_socket | c_long::from(X32_SYSCALL_BIT),
        family,
        sock_type,
    )
}

fn probe_socket_nr(nr: c_long, family: u64, sock_type: u64) -> Result<(), i32> {
    // The kernel reads `int` arguments from the low 32 bits of the register;
    // the wrapping cast here is exactly that register.
    #[allow(clippy::cast_possible_wrap)]
    let (family, sock_type) = (family as c_long, sock_type as c_long);
    // SAFETY: `socket` takes three integers and returns a descriptor or -1;
    // no pointer is involved.
    let fd = unsafe { libc::syscall(nr, family, sock_type, 0 as c_long) };
    if fd < 0 {
        return Err(errno());
    }
    // SAFETY: the descriptor was just created by this probe and is not shared.
    unsafe {
        libc::close(c_int_from(fd));
    }
    Ok(())
}

/// `io_uring_setup(1, NULL)` as a probe. Unfiltered, the kernel answers
/// `EFAULT` for the null pointer; filtered, `EPERM` before it looks.
pub fn probe_io_uring_setup() -> Result<(), i32> {
    // SAFETY: a null parameter pointer is rejected by the kernel with EFAULT
    // (or by the filter with EPERM) and never dereferenced by us.
    let rc = unsafe {
        libc::syscall(
            libc::SYS_io_uring_setup,
            1 as c_uint,
            std::ptr::null_mut::<libc::c_void>(),
        )
    };
    if rc < 0 {
        return Err(errno());
    }
    // SAFETY: a ring descriptor the kernel just handed out (the filter is
    // missing); closing it keeps the probe side-effect free.
    unsafe {
        libc::close(c_int_from(rc));
    }
    Ok(())
}

/// The `Seccomp:` line of `/proc/self/status` (0 disabled, 1 strict, 2
/// filter); `None` when `/proc` cannot be read.
#[must_use]
pub fn seccomp_mode() -> Option<u32> {
    status_field(b"\nSeccomp:")
}

/// The `NoNewPrivs:` line of `/proc/self/status`; `None` when `/proc`
/// cannot be read.
#[must_use]
pub fn no_new_privs() -> Option<u32> {
    status_field(b"\nNoNewPrivs:")
}

/// Reads one numeric field of `/proc/self/status` without allocating, so a
/// freshly forked child of a multi-threaded process may call it.
fn status_field(key: &[u8]) -> Option<u32> {
    let mut buf = [0u8; 8192];
    // SAFETY: the path is a NUL-terminated literal; the flags are constants.
    let fd = unsafe {
        libc::open(
            c"/proc/self/status".as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC,
        )
    };
    if fd < 0 {
        return None;
    }
    let mut len = 0usize;
    while len < buf.len() {
        // SAFETY: the pointer and length describe the unused tail of `buf`.
        let n = unsafe { libc::read(fd, buf[len..].as_mut_ptr().cast(), buf.len() - len) };
        match usize::try_from(n) {
            Ok(0) | Err(_) => break,
            Ok(n) => len += n,
        }
    }
    // SAFETY: `fd` was opened above and is not shared.
    unsafe {
        libc::close(fd);
    }
    let text = &buf[..len];
    let start = text.windows(key.len()).position(|w| w == key)? + key.len();
    let mut value: u32 = 0;
    let mut seen_digit = false;
    for &byte in &text[start..] {
        match byte {
            b' ' | b'\t' if !seen_digit => {}
            b'0'..=b'9' => {
                seen_digit = true;
                value = value.checked_mul(10)?.checked_add(u32::from(byte - b'0'))?;
            }
            _ => break,
        }
    }
    seen_digit.then_some(value)
}

fn errno() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

fn c_int_from(value: c_long) -> libc::c_int {
    libc::c_int::try_from(value).unwrap_or(-1)
}

fn family(name: &str) -> Result<Family, Error> {
    FAMILIES
        .iter()
        .copied()
        .find(|family| family.name == name)
        .ok_or_else(|| Error::UnknownFamily(name.to_owned()))
}

fn sock_type(name: &str) -> Result<SockType, Error> {
    SOCK_TYPES
        .iter()
        .copied()
        .find(|sock_type| sock_type.name == name)
        .ok_or_else(|| Error::UnknownType(name.to_owned()))
}

fn syscall(name: &str) -> Result<Syscall, Error> {
    SYSCALLS
        .iter()
        .copied()
        .find(|syscall| syscall.name == name)
        .ok_or_else(|| Error::UnknownSyscall(name.to_owned()))
}

fn push_unique<T: PartialEq>(list: &mut Vec<T>, item: T) {
    if !list.contains(&item) {
        list.push(item);
    }
}

/// A comma list from the environment; empty items are skipped, duplicates
/// dropped, an empty result is an error.
fn parse_list<T: PartialEq>(
    text: &str,
    var: &'static str,
    lookup: fn(&str) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let mut out = Vec::new();
    for item in text.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        push_unique(&mut out, lookup(item)?);
    }
    if out.is_empty() {
        return Err(Error::EmptyList(var));
    }
    Ok(out)
}

const fn stmt(code: u16, k: u32) -> sock_filter {
    sock_filter {
        code,
        jt: 0,
        jf: 0,
        k,
    }
}

const fn jump(code: u16, k: u32, jt: u8, jf: u8) -> sock_filter {
    sock_filter { code, jt, jf, k }
}

/// The hand-written prelude. Layout, with `F` families and `T` types:
///
/// ```text
/// 0      ld   arch
/// 1      jeq  AUDIT_ARCH        -> 3, else 2
/// 2      ret  KILL_PROCESS
/// 3      ld   nr
/// 4      jset X32_SYSCALL_BIT   -> 5, else 6
/// 5      ret  EPERM
/// 6      jeq  SYS_seccomp       -> 7, else 10
/// 7      ld   arg1 (low word)
/// 8      jset NEW_LISTENER      -> 9, else 10
/// 9      ret  EPERM
/// 10     ld   nr
/// 11     jeq  SYS_socket        -> 12, else past the gate (seccompiler program)
/// 12     ld   arg0 (low word)
/// 13+i   jeq  family[i]         -> 14+F (type check), else next
/// 13+F   ret  REFUSE
/// 14+F   ld   arg1 (low word)
/// 15+F   and  0xff
/// 16+F+i jeq  type[i]           -> 17+F+T (seccompiler program), else next
/// 16+F+T ret  REFUSE
/// 17+F+T <seccompiler program>
/// ```
///
/// `REFUSE` is `EPERM` or `USER_NOTIF`, as `gate` says; the x32 answer at 5
/// and the listener answer at 9 are `EPERM` either way.
///
/// Lines 6 to 10 keep the agent from installing a filter with a listener of
/// its own (`SECCOMP_FILTER_FLAG_NEW_LISTENER`, HUM-138). The kernel allows
/// one listener per filter chain; while the shim's parent holds the one of
/// the agent's filter, a second is refused anyway (`EBUSY`), but a listener
/// that closed would lift that, and the agent's own, newer filter could then
/// answer its refused sockets with "go ahead". `prctl(PR_SET_SECCOMP)` cannot
/// set flags, so this is the only way to ask for a listener, and the rule
/// stands in both gates, whatever becomes of the parent.
///
/// Every jump is relative, so the seccompiler program that follows needs no
/// relocation; it starts with its own architecture check and reloads `nr`.
fn prelude(families: &[Family], types: &[SockType], gate: Gate) -> Result<Vec<sock_filter>, Error> {
    let n_f = u8::try_from(families.len()).map_err(|_| Error::TooManyEntries)?;
    let n_t = u8::try_from(types.len()).map_err(|_| Error::TooManyEntries)?;
    let past_gate = n_f
        .checked_add(n_t)
        .and_then(|n| n.checked_add(5))
        .ok_or(Error::TooManyEntries)?;

    let mut out = Vec::with_capacity(PRELUDE_FIXED + families.len() + types.len());
    out.push(stmt(BPF_LD_W_ABS, DATA_ARCH));
    out.push(jump(BPF_JMP_JEQ_K, AUDIT_ARCH, 1, 0));
    out.push(stmt(BPF_RET_K, RET_KILL_PROCESS));
    out.push(stmt(BPF_LD_W_ABS, DATA_NR));
    out.push(jump(BPF_JMP_JSET_K, X32_SYSCALL_BIT, 0, 1));
    out.push(stmt(BPF_RET_K, RET_EPERM));
    out.push(jump(
        BPF_JMP_JEQ_K,
        u32::try_from(libc::SYS_seccomp).map_err(|_| Error::TooManyEntries)?,
        0,
        3,
    ));
    out.push(stmt(BPF_LD_W_ABS, DATA_ARG1_LOW));
    out.push(jump(BPF_JMP_JSET_K, NEW_LISTENER, 0, 1));
    out.push(stmt(BPF_RET_K, RET_EPERM));
    out.push(stmt(BPF_LD_W_ABS, DATA_NR));
    out.push(jump(
        BPF_JMP_JEQ_K,
        u32::try_from(libc::SYS_socket).map_err(|_| Error::TooManyEntries)?,
        0,
        past_gate,
    ));
    out.push(stmt(BPF_LD_W_ABS, DATA_ARG0_LOW));
    for (i, family) in (0u8..).zip(families) {
        out.push(jump(BPF_JMP_JEQ_K, family.number, n_f - i, 0));
    }
    out.push(stmt(BPF_RET_K, gate.refuse()));
    out.push(stmt(BPF_LD_W_ABS, DATA_ARG1_LOW));
    out.push(stmt(BPF_ALU_AND_K, SOCK_TYPE_MASK));
    for (i, sock_type) in (0u8..).zip(types) {
        out.push(jump(BPF_JMP_JEQ_K, sock_type.number, n_t - i, 0));
    }
    out.push(stmt(BPF_RET_K, gate.refuse()));
    Ok(out)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss
    )]

    use super::*;

    const AF_UNIX: u64 = libc::AF_UNIX as u64;
    const AF_INET: u64 = libc::AF_INET as u64;
    const AF_INET6: u64 = libc::AF_INET6 as u64;
    const AF_NETLINK: u64 = libc::AF_NETLINK as u64;
    const SOCK_STREAM: u64 = libc::SOCK_STREAM as u64;
    const SOCK_DGRAM: u64 = libc::SOCK_DGRAM as u64;
    const SOCK_RAW: u64 = libc::SOCK_RAW as u64;
    const SOCK_CLOEXEC: u64 = libc::SOCK_CLOEXEC as u64;
    const SOCK_NONBLOCK: u64 = libc::SOCK_NONBLOCK as u64;
    const EPERM: i32 = libc::EPERM;
    const EINVAL: i32 = libc::EINVAL;
    /// `AUDIT_ARCH_I386`: what a 32-bit binary would present on `x86_64`.
    const FOREIGN_ARCH: u32 = 3 | 0x4000_0000;

    fn default_policy() -> Policy {
        Policy::from_env(None, None, None).unwrap()
    }

    // ---- opcode pins and the table --------------------------------------

    #[test]
    fn opcodes_match_libc() {
        assert_eq!(
            u32::from(BPF_LD_W_ABS),
            libc::BPF_LD | libc::BPF_W | libc::BPF_ABS
        );
        assert_eq!(
            u32::from(BPF_ALU_AND_K),
            libc::BPF_ALU | libc::BPF_AND | libc::BPF_K
        );
        assert_eq!(
            u32::from(BPF_JMP_JEQ_K),
            libc::BPF_JMP | libc::BPF_JEQ | libc::BPF_K
        );
        assert_eq!(
            u32::from(BPF_JMP_JSET_K),
            libc::BPF_JMP | libc::BPF_JSET | libc::BPF_K
        );
        assert_eq!(u32::from(BPF_RET_K), libc::BPF_RET | libc::BPF_K);
        assert_eq!(RET_EPERM, libc::SECCOMP_RET_ERRNO | 1);
        assert_eq!(RET_KILL_PROCESS, 0x8000_0000);
        assert_eq!(
            usize::try_from(DATA_ARG0_LOW).unwrap(),
            std::mem::offset_of!(libc::seccomp_data, args)
        );
        assert_eq!(std::mem::offset_of!(libc::seccomp_data, arch), 4);
    }

    /// The table `docs/SECURITY.md` cites, row by row, for the default
    /// profile and the agent's gate. A change here is a change to guarantee
    /// three: update the document in the same commit.
    #[test]
    fn rule_table_lists_every_rule() {
        let rows: Vec<String> = default_policy()
            .rules(Gate::Reported)
            .iter()
            .map(ToString::to_string)
            .collect();
        for row in &rows {
            println!("{row}");
        }
        let arch = std::env::consts::ARCH;
        let expected = [
            format!("every syscall      | arch is not {arch:<34} | kill process | prelude"),
            "every syscall      | nr has the x32 bit 0x40000000                  | EPERM        | prelude".to_owned(),
            "seccomp            | flags (arg1) have NEW_LISTENER 0x8             | EPERM        | prelude".to_owned(),
            "socket             | family (arg0) not in {AF_INET, AF_INET6}       | notify:EPERM | prelude".to_owned(),
            "socket             | type (arg1 & 0xff) not in {SOCK_STREAM}        | notify:EPERM | prelude".to_owned(),
            "ptrace             | always                                         | EPERM        | seccompiler".to_owned(),
            "io_uring_setup     | always                                         | EPERM        | seccompiler".to_owned(),
            "io_uring_enter     | always                                         | EPERM        | seccompiler".to_owned(),
            "io_uring_register  | always                                         | EPERM        | seccompiler".to_owned(),
            "process_vm_readv   | always                                         | EPERM        | seccompiler".to_owned(),
            "process_vm_writev  | always                                         | EPERM        | seccompiler".to_owned(),
            "pidfd_getfd        | always                                         | EPERM        | seccompiler".to_owned(),
            "keyctl             | always                                         | EPERM        | seccompiler".to_owned(),
            "add_key            | always                                         | EPERM        | seccompiler".to_owned(),
            "request_key        | always                                         | EPERM        | seccompiler".to_owned(),
            "kexec_load         | always                                         | EPERM        | seccompiler".to_owned(),
            "kexec_file_load    | always                                         | EPERM        | seccompiler".to_owned(),
            "init_module        | always                                         | EPERM        | seccompiler".to_owned(),
            "finit_module       | always                                         | EPERM        | seccompiler".to_owned(),
            "delete_module      | always                                         | EPERM        | seccompiler".to_owned(),
            "bpf                | always                                         | EPERM        | seccompiler".to_owned(),
            "perf_event_open    | always                                         | EPERM        | seccompiler".to_owned(),
            "userfaultfd        | always                                         | EPERM        | seccompiler".to_owned(),
        ];
        assert_eq!(rows, expected);
        // The bridge's gate answers the same two rows with a plain EPERM.
        let silent: Vec<String> = default_policy()
            .rules(Gate::Silent)
            .iter()
            .map(ToString::to_string)
            .collect();
        assert_eq!(
            silent[4],
            expected[4].replace("notify:EPERM", "EPERM       ")
        );
        assert_eq!(
            silent[3],
            expected[3].replace("notify:EPERM", "EPERM       ")
        );
        assert_eq!(silent[5..], expected[5..]);
        assert_eq!(silent[..3], expected[..3]);
        // socketpair is deliberately absent: no rule, hence allowed.
        assert!(
            !default_policy()
                .rules(Gate::Reported)
                .iter()
                .any(|rule| matches!(&rule.subject, Subject::Named(s) if s.name == "socketpair"))
        );
    }

    #[test]
    fn floor_is_in_the_table_and_first_in_every_policy() {
        for name in FLOOR {
            syscall(name).unwrap();
        }
        let policy = Policy::from_env(None, None, Some("bpf,ptrace,mount,chroot")).unwrap();
        let names: Vec<&str> = policy.deny.iter().map(|s| s.name).collect();
        assert_eq!(&names[..FLOOR.len()], FLOOR);
        // `bpf` and `ptrace` are already on the floor and are not repeated.
        assert_eq!(&names[FLOOR.len()..], ["mount", "chroot"]);
    }

    /// The hardening syscalls of the table in `backlog/sprint-1.md` are the
    /// floor, not an option a profile may leave out.
    #[test]
    fn the_hardening_syscalls_are_on_the_floor() {
        for name in [
            "kexec_load",
            "kexec_file_load",
            "init_module",
            "finit_module",
            "delete_module",
            "bpf",
            "perf_event_open",
            "userfaultfd",
        ] {
            assert!(FLOOR.contains(&name), "{name} is missing from the floor");
        }
        let policy = default_policy();
        for name in FLOOR {
            assert!(
                policy.deny.iter().any(|syscall| syscall.name == *name),
                "{name} is missing from the default policy"
            );
        }
        assert_eq!(policy.deny.len(), FLOOR.len());
    }

    #[test]
    fn every_table_entry_has_a_distinct_name_and_number() {
        for (i, a) in SYSCALLS.iter().enumerate() {
            for b in &SYSCALLS[i + 1..] {
                assert_ne!(a.name, b.name);
                assert_ne!(a.nr, b.nr, "{} and {} share a number", a.name, b.name);
            }
        }
    }

    // ---- the environment contract ----------------------------------------

    #[test]
    fn unknown_names_are_errors_not_warnings() {
        assert!(matches!(
            Policy::from_env(Some("AF_INET,AF_BOGUS"), None, None),
            Err(Error::UnknownFamily(name)) if name == "AF_BOGUS"
        ));
        assert!(matches!(
            Policy::from_env(None, Some("SOCK_RAW"), None),
            Err(Error::UnknownType(name)) if name == "SOCK_RAW"
        ));
        assert!(matches!(
            Policy::from_env(None, None, Some("ptrace, frobnicate")),
            Err(Error::UnknownSyscall(name)) if name == "frobnicate"
        ));
    }

    #[test]
    fn set_but_empty_lists_are_errors_absent_means_default() {
        assert!(matches!(
            Policy::from_env(Some(""), None, None),
            Err(Error::EmptyList("HUMANITL_SECCOMP_FAMILIES"))
        ));
        assert!(matches!(
            Policy::from_env(None, Some(" , "), None),
            Err(Error::EmptyList("HUMANITL_SECCOMP_TYPES"))
        ));
        let policy = Policy::from_env(None, None, Some("")).unwrap();
        assert_eq!(policy.deny.len(), FLOOR.len());
        let policy = default_policy();
        assert_eq!(
            policy.families().iter().map(|f| f.name).collect::<Vec<_>>(),
            ["AF_INET", "AF_INET6"]
        );
        assert_eq!(
            policy.types().iter().map(|t| t.name).collect::<Vec<_>>(),
            ["SOCK_STREAM"]
        );
    }

    #[test]
    fn lists_are_trimmed_and_deduplicated() {
        let policy = Policy::from_env(
            Some(" AF_INET6 ,AF_INET,AF_INET6"),
            Some("SOCK_STREAM,SOCK_STREAM"),
            None,
        )
        .unwrap();
        assert_eq!(
            policy.families().iter().map(|f| f.name).collect::<Vec<_>>(),
            ["AF_INET6", "AF_INET"]
        );
        assert_eq!(policy.types().len(), 1);
    }

    #[test]
    fn bridge_policy_adds_unix_and_nothing_else() {
        let agent = default_policy();
        let bridge = agent.for_bridge();
        assert_eq!(
            bridge.families().iter().map(|f| f.name).collect::<Vec<_>>(),
            ["AF_INET", "AF_INET6", "AF_UNIX"]
        );
        assert_eq!(bridge.types(), agent.types());
        assert_eq!(bridge.deny, agent.deny);
        // Idempotent when the profile already allows AF_UNIX.
        let browser = Policy::from_env(Some("AF_UNIX,AF_INET"), None, None).unwrap();
        assert_eq!(browser.for_bridge().families().len(), 2);
    }

    // ---- the program, evaluated by an interpreter -------------------------

    /// A classic-BPF interpreter for the subset seccompiler and the prelude
    /// emit. Independent of the kernel, so it can drive the foreign-arch case
    /// and every argument combination without forking.
    fn evaluate(program: &[sock_filter], nr: u32, arch: u32, args: [u64; 6]) -> u32 {
        let load = |offset: u32| -> u32 {
            match offset {
                0 => nr,
                4 => arch,
                8 | 12 => 0,
                16..=63 => {
                    let index = usize::try_from((offset - 16) / 8).unwrap();
                    let high = (offset - 16) % 8 == 4;
                    let value = args[index];
                    u32::try_from(if high {
                        value >> 32
                    } else {
                        value & 0xffff_ffff
                    })
                    .unwrap()
                }
                other => panic!("load from offset {other}"),
            }
        };
        let mut pc = 0usize;
        let mut acc = 0u32;
        for _ in 0..10_000 {
            let ins = &program[pc];
            let (jt, jf) = (usize::from(ins.jt), usize::from(ins.jf));
            match ins.code {
                0x20 => {
                    acc = load(ins.k);
                    pc += 1;
                }
                0x54 => {
                    acc &= ins.k;
                    pc += 1;
                }
                0x05 => pc += 1 + usize::try_from(ins.k).unwrap(),
                0x15 => pc += 1 + if acc == ins.k { jt } else { jf },
                0x25 => pc += 1 + if acc > ins.k { jt } else { jf },
                0x35 => pc += 1 + if acc >= ins.k { jt } else { jf },
                0x45 => pc += 1 + if acc & ins.k != 0 { jt } else { jf },
                0x06 => return ins.k,
                other => panic!("opcode {other:#x} at {pc} is not in the subset"),
            }
        }
        panic!("the program does not return")
    }

    const ALLOW: u32 = libc::SECCOMP_RET_ALLOW;
    const KILL: u32 = libc::SECCOMP_RET_KILL_PROCESS;
    const NR_SOCKET: u32 = libc::SYS_socket as u32;
    const NR_SOCKETPAIR: u32 = libc::SYS_socketpair as u32;
    const NR_READ: u32 = libc::SYS_read as u32;

    #[test]
    fn program_decides_socket_by_family_and_masked_type() {
        let program = default_policy().program(Gate::Silent).unwrap();
        let socket = |family: u64, sock_type: u64| {
            evaluate(
                &program,
                NR_SOCKET,
                AUDIT_ARCH,
                [family, sock_type, 0, 0, 0, 0],
            )
        };
        assert_eq!(socket(AF_INET, SOCK_STREAM), ALLOW);
        assert_eq!(
            socket(AF_INET6, SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC),
            ALLOW
        );
        assert_eq!(socket(AF_UNIX, SOCK_STREAM), RET_EPERM);
        assert_eq!(socket(AF_NETLINK, SOCK_RAW), RET_EPERM);
        assert_eq!(socket(AF_INET, SOCK_DGRAM), RET_EPERM);
        assert_eq!(socket(AF_INET, SOCK_DGRAM | SOCK_CLOEXEC), RET_EPERM);
        assert_eq!(socket(AF_INET, SOCK_RAW), RET_EPERM);
        // The kernel reads the low 32 bits of an int argument; so does the gate.
        assert_eq!(socket(AF_INET | (1 << 32), SOCK_STREAM), ALLOW);
        assert_eq!(socket(AF_UNIX | (1 << 32), SOCK_STREAM), RET_EPERM);
        // A flag bit outside the mask is the kernel's problem (EINVAL), not a
        // type the gate mistakes for something else.
        assert_eq!(socket(AF_INET, SOCK_STREAM | 0x100), ALLOW);
        assert_eq!(socket(AF_INET, SOCK_DGRAM | 0x100), RET_EPERM);
    }

    #[test]
    fn program_refuses_x32_numbers_and_kills_foreign_architectures() {
        let program = default_policy().program(Gate::Silent).unwrap();
        let args = [AF_INET, SOCK_STREAM, 0, 0, 0, 0];
        assert_eq!(
            evaluate(&program, NR_SOCKET | X32_SYSCALL_BIT, AUDIT_ARCH, args),
            RET_EPERM
        );
        assert_eq!(
            evaluate(&program, NR_READ | X32_SYSCALL_BIT, AUDIT_ARCH, args),
            RET_EPERM
        );
        assert_eq!(evaluate(&program, NR_SOCKET, FOREIGN_ARCH, args), KILL);
        assert_eq!(evaluate(&program, NR_READ, 0, args), KILL);
    }

    /// The agent's gate parks exactly the two refusals of the socket gate and
    /// leaves every other answer to the kernel (HUM-138).
    #[test]
    fn reported_gate_parks_refused_sockets_and_nothing_else() {
        let program = default_policy().program(Gate::Reported).unwrap();
        let socket = |family: u64, sock_type: u64| {
            evaluate(
                &program,
                NR_SOCKET,
                AUDIT_ARCH,
                [family, sock_type, 0, 0, 0, 0],
            )
        };
        assert_eq!(socket(AF_UNIX, SOCK_STREAM), RET_USER_NOTIF);
        assert_eq!(socket(AF_INET, SOCK_DGRAM | SOCK_CLOEXEC), RET_USER_NOTIF);
        assert_eq!(socket(AF_INET6, SOCK_STREAM | SOCK_NONBLOCK), ALLOW);
        let unix = [AF_UNIX, SOCK_STREAM, 0, 0, 0, 0];
        assert_eq!(
            evaluate(&program, NR_SOCKET | X32_SYSCALL_BIT, AUDIT_ARCH, unix),
            RET_EPERM
        );
        assert_eq!(
            evaluate(&program, libc::SYS_ptrace as u32, AUDIT_ARCH, [0; 6]),
            RET_EPERM
        );
        assert_eq!(evaluate(&program, NR_SOCKET, FOREIGN_ARCH, unix), KILL);
        // Only the two refusal slots differ from the bridge's gate.
        let silent = default_policy().program(Gate::Silent).unwrap();
        let differing: Vec<usize> = (0..program.len())
            .filter(|&i| program[i] != silent[i])
            .collect();
        assert_eq!(differing, [15, 19], "{differing:?}");
    }

    /// Neither gate lets the agent ask for a listener of its own (HUM-138):
    /// with one, a newer filter of the agent could answer its refused
    /// sockets with "go ahead" once the parent's listener is gone.
    #[test]
    fn no_gate_hands_out_a_listener() {
        const NR_SECCOMP: u32 = libc::SYS_seccomp as u32;
        const SET_MODE_FILTER: u64 = libc::SECCOMP_SET_MODE_FILTER as u64;
        let listener = libc::SECCOMP_FILTER_FLAG_NEW_LISTENER;
        let tsync = libc::SECCOMP_FILTER_FLAG_TSYNC;
        for gate in [Gate::Silent, Gate::Reported] {
            let program = default_policy().program(gate).unwrap();
            let seccomp = |flags: u64| {
                evaluate(
                    &program,
                    NR_SECCOMP,
                    AUDIT_ARCH,
                    [SET_MODE_FILTER, flags, 0, 0, 0, 0],
                )
            };
            assert_eq!(seccomp(listener), RET_EPERM, "{gate:?}");
            assert_eq!(seccomp(listener | tsync), RET_EPERM, "{gate:?}");
            // The high word does not hide the flag: the kernel reads an
            // `unsigned int`.
            assert_eq!(seccomp(listener | (1 << 32)), RET_EPERM, "{gate:?}");
            // A filter without a listener stays the agent's own business.
            assert_eq!(seccomp(tsync), ALLOW, "{gate:?}");
            assert_eq!(seccomp(0), ALLOW, "{gate:?}");
        }
    }

    #[test]
    fn program_denies_the_floor_and_allows_the_rest() {
        let policy = default_policy();
        let program = policy.program(Gate::Silent).unwrap();
        let args = [0; 6];
        for syscall in policy.deny {
            let nr = u32::try_from(syscall.nr).unwrap();
            assert_eq!(
                evaluate(&program, nr, AUDIT_ARCH, args),
                RET_EPERM,
                "{}",
                syscall.name
            );
        }
        assert_eq!(evaluate(&program, NR_READ, AUDIT_ARCH, args), ALLOW);
        assert_eq!(
            evaluate(
                &program,
                NR_SOCKETPAIR,
                AUDIT_ARCH,
                [AF_UNIX, SOCK_STREAM, 0, 0, 0, 0]
            ),
            ALLOW
        );
        assert_eq!(
            evaluate(&program, libc::SYS_bpf as u32, AUDIT_ARCH, args),
            RET_EPERM,
            "bpf is on the floor, whatever the profile says"
        );
        assert_eq!(
            evaluate(&program, libc::SYS_userfaultfd as u32, AUDIT_ARCH, args),
            RET_EPERM,
            "userfaultfd is on the floor, whatever the profile says"
        );
        // A syscall a profile could add but neither the floor nor the default
        // profile names stays allowed.
        assert_eq!(
            evaluate(&program, libc::SYS_mount as u32, AUDIT_ARCH, args),
            ALLOW
        );
    }

    #[test]
    fn program_honours_a_profile_that_widens_families_and_types() {
        let policy = Policy::from_env(
            Some("AF_INET,AF_INET6,AF_UNIX"),
            Some("SOCK_STREAM,SOCK_DGRAM"),
            Some("mount"),
        )
        .unwrap();
        let program = policy.program(Gate::Silent).unwrap();
        let socket = |family: u64, sock_type: u64| {
            evaluate(
                &program,
                NR_SOCKET,
                AUDIT_ARCH,
                [family, sock_type, 0, 0, 0, 0],
            )
        };
        assert_eq!(socket(AF_UNIX, SOCK_DGRAM), ALLOW);
        assert_eq!(socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC), ALLOW);
        assert_eq!(socket(AF_UNIX, SOCK_RAW), RET_EPERM);
        assert_eq!(socket(AF_NETLINK, SOCK_DGRAM), RET_EPERM);
        assert_eq!(
            evaluate(&program, libc::SYS_mount as u32, AUDIT_ARCH, [0; 6]),
            RET_EPERM
        );
        // The floor stays under the profile's own list.
        assert_eq!(
            evaluate(&program, libc::SYS_bpf as u32, AUDIT_ARCH, [0; 6]),
            RET_EPERM
        );
    }

    #[test]
    fn denying_socket_by_name_beats_the_gate() {
        let policy = Policy::from_env(None, None, Some("socket")).unwrap();
        let program = policy.program(Gate::Silent).unwrap();
        assert_eq!(
            evaluate(
                &program,
                NR_SOCKET,
                AUDIT_ARCH,
                [AF_INET, SOCK_STREAM, 0, 0, 0, 0]
            ),
            RET_EPERM
        );
    }

    #[test]
    fn prelude_has_the_documented_layout() {
        let policy = default_policy();
        let prelude = prelude(policy.families(), policy.types(), Gate::Silent).unwrap();
        // The fixed instructions plus one per family and type.
        assert_eq!(prelude.len(), PRELUDE_FIXED + 2 + 1);
        assert_eq!(prelude[0], stmt(BPF_LD_W_ABS, DATA_ARCH));
        assert_eq!(prelude[2].k, RET_KILL_PROCESS);
        assert_eq!(prelude[4].code, BPF_JMP_JSET_K);
        assert_eq!(prelude[4].k, X32_SYSCALL_BIT);
        assert_eq!(prelude[6].k, libc::SYS_seccomp as u32);
        assert_eq!(prelude[8].code, BPF_JMP_JSET_K);
        assert_eq!(prelude[8].k, NEW_LISTENER);
        assert_eq!(prelude[9].k, RET_EPERM);
        assert_eq!(prelude[11].k, NR_SOCKET);
        assert_eq!(usize::from(prelude[11].jf), prelude.len() - 12);
        assert_eq!(prelude[15 + 2].k, SOCK_TYPE_MASK);
        assert_eq!(prelude.last().unwrap().k, RET_EPERM);
        let whole = policy.program(Gate::Silent).unwrap();
        assert_eq!(&whole[..prelude.len()], &prelude[..]);
        assert!(whole.len() < 4096);
    }

    // ---- the program, installed in a forked child ------------------------

    /// Runs `probe` in a forked child that carries `program`, and returns
    /// the child's exit status: the probe's result, 0 for success and the
    /// errno otherwise. 254 means the filter could not be installed, 255 that
    /// the child died of a signal.
    ///
    /// The test harness is multi-threaded, so the child touches no allocator:
    /// the program is built before the fork and the probes are plain `fn`s
    /// over libc.
    fn in_filtered_child(program: &BpfProgram, probe: fn() -> i32) -> i32 {
        // SAFETY: the child calls only async-signal-safe functions (prctl,
        // seccomp, socket, close, syscall, open, read, _exit) before _exit.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0, "fork failed");
        if pid == 0 {
            let code = match apply(program) {
                Ok(()) => probe() & 0xff,
                Err(_) => 254,
            };
            // SAFETY: _exit ends the child without running the harness's
            // destructors.
            unsafe { libc::_exit(code) }
        }
        let mut status = 0;
        // SAFETY: waitpid on the child we just created, with a valid pointer.
        let waited = unsafe { libc::waitpid(pid, &raw mut status, 0) };
        assert_eq!(waited, pid);
        if libc::WIFEXITED(status) {
            libc::WEXITSTATUS(status)
        } else {
            255
        }
    }

    fn code(result: Result<(), i32>) -> i32 {
        match result {
            Ok(()) => 0,
            Err(errno) => errno,
        }
    }

    fn default_program() -> BpfProgram {
        default_policy().program(Gate::Silent).unwrap()
    }

    /// `seccomp(SET_MODE_FILTER, NEW_LISTENER, prog)` with a one-line
    /// "allow" program: 0 when the kernel handed out a listener (closed at
    /// once), otherwise the errno.
    fn try_own_listener() -> i32 {
        let allow = [sock_filter {
            code: BPF_RET_K,
            jt: 0,
            jf: 0,
            k: libc::SECCOMP_RET_ALLOW,
        }];
        let fprog = libc::sock_fprog {
            len: 1,
            filter: allow.as_ptr().cast_mut().cast::<libc::sock_filter>(),
        };
        // SAFETY: `fprog` points at `allow`, which outlives the call.
        let rc = unsafe {
            libc::syscall(
                libc::SYS_seccomp,
                libc::c_ulong::from(libc::SECCOMP_SET_MODE_FILTER),
                libc::SECCOMP_FILTER_FLAG_NEW_LISTENER,
                &raw const fprog,
            )
        };
        if rc < 0 {
            return errno();
        }
        // SAFETY: a listener the kernel just handed out.
        unsafe {
            libc::close(c_int_from(rc));
        }
        0
    }

    /// The attack of the review on HUM-138, measured: the agent's filter
    /// carries a listener, the listener is gone (the parent died, or gave
    /// up), and the agent asks for a listener of its own. Without the
    /// prelude's rule the kernel would hand one out, because only a live
    /// listener makes it refuse (`EBUSY`).
    #[test]
    fn a_dropped_listener_does_not_free_the_agent_to_take_one() {
        // Built before the fork, so the child allocates nothing: the harness
        // is multi-threaded. Not `in_filtered_child`, which would install the
        // filter without a listener.
        let program = default_policy().program(Gate::Reported).unwrap();
        // SAFETY: the child calls prctl, seccomp, close, syscall and _exit.
        let pid = unsafe { libc::fork() };
        assert!(pid >= 0);
        if pid == 0 {
            let code = match apply_reporting(&program) {
                Ok(listener) => {
                    drop(listener);
                    try_own_listener() & 0xff
                }
                Err(_) => 251,
            };
            // SAFETY: _exit without the harness's destructors.
            unsafe { libc::_exit(code) }
        }
        let mut status = 0;
        // SAFETY: our own child, valid pointer.
        assert_eq!(unsafe { libc::waitpid(pid, &raw mut status, 0) }, pid);
        assert!(libc::WIFEXITED(status));
        assert_eq!(libc::WEXITSTATUS(status), EPERM);
        // Under the bridge's gate as well.
        let silent = default_policy().program(Gate::Silent).unwrap();
        assert_eq!(in_filtered_child(&silent, try_own_listener), EPERM);
    }

    #[test]
    fn filter_denies_unix() {
        fn probe() -> i32 {
            code(probe_socket(AF_UNIX, SOCK_STREAM))
        }
        assert_eq!(in_filtered_child(&default_program(), probe), EPERM);
    }

    #[test]
    fn filter_allows_inet_stream_with_flags() {
        fn probe() -> i32 {
            code(probe_socket(AF_INET, SOCK_STREAM | SOCK_CLOEXEC))
        }
        assert_eq!(in_filtered_child(&default_program(), probe), 0);
    }

    #[test]
    fn filter_allows_inet6_stream_nonblock() {
        fn probe() -> i32 {
            code(probe_socket(
                AF_INET6,
                SOCK_STREAM | SOCK_NONBLOCK | SOCK_CLOEXEC,
            ))
        }
        let result = in_filtered_child(&default_program(), probe);
        assert!(
            result == 0 || result == libc::EAFNOSUPPORT,
            "expected success or a kernel without IPv6, got errno {result}"
        );
    }

    #[test]
    fn filter_denies_inet_dgram() {
        fn probe() -> i32 {
            code(probe_socket(AF_INET, SOCK_DGRAM))
        }
        assert_eq!(in_filtered_child(&default_program(), probe), EPERM);
    }

    #[test]
    fn filter_denies_inet_dgram_with_flags() {
        fn probe() -> i32 {
            code(probe_socket(AF_INET, SOCK_DGRAM | SOCK_CLOEXEC))
        }
        assert_eq!(in_filtered_child(&default_program(), probe), EPERM);
    }

    #[test]
    fn filter_reads_the_low_word_of_the_family() {
        fn probe() -> i32 {
            code(probe_socket(AF_UNIX | (1 << 32), SOCK_STREAM))
        }
        assert_eq!(in_filtered_child(&default_program(), probe), EPERM);
    }

    #[test]
    fn filter_mask_leaves_unknown_flags_to_the_kernel() {
        fn probe() -> i32 {
            code(probe_socket(AF_INET, SOCK_STREAM | 0x100))
        }
        assert_eq!(in_filtered_child(&default_program(), probe), EINVAL);
    }

    #[test]
    fn filter_allows_socketpair() {
        fn probe() -> i32 {
            let mut fds = [0 as libc::c_int; 2];
            // SAFETY: `fds` is a valid two-element array for socketpair.
            let rc =
                unsafe { libc::socketpair(libc::AF_UNIX, libc::SOCK_STREAM, 0, fds.as_mut_ptr()) };
            if rc != 0 {
                return errno();
            }
            // SAFETY: both descriptors were just created.
            unsafe {
                libc::close(fds[0]);
                libc::close(fds[1]);
            }
            0
        }
        assert_eq!(in_filtered_child(&default_program(), probe), 0);
    }

    #[test]
    fn filter_denies_io_uring_setup() {
        fn probe() -> i32 {
            code(probe_io_uring_setup())
        }
        assert_eq!(in_filtered_child(&default_program(), probe), EPERM);
    }

    #[test]
    fn filter_denies_ptrace() {
        fn probe() -> i32 {
            // PTRACE_TRACEME with no other argument.
            // SAFETY: four integer arguments, no pointer.
            let rc = unsafe {
                libc::syscall(
                    libc::SYS_ptrace,
                    0 as c_long,
                    0 as c_long,
                    0 as c_long,
                    0 as c_long,
                )
            };
            if rc < 0 { errno() } else { 0 }
        }
        assert_eq!(in_filtered_child(&default_program(), probe), EPERM);
    }

    #[test]
    fn filter_denies_x32_socket() {
        fn probe() -> i32 {
            code(probe_socket_x32(AF_INET, SOCK_STREAM))
        }
        assert_eq!(in_filtered_child(&default_program(), probe), EPERM);
    }

    #[test]
    fn filter_reports_seccomp_mode_2_and_no_new_privs() {
        fn probe() -> i32 {
            match (seccomp_mode(), no_new_privs()) {
                (Some(2), Some(1)) => 0,
                (Some(mode), Some(nnp)) => 100 + i32::try_from(mode * 10 + nnp).unwrap_or(99),
                _ => 200,
            }
        }
        assert_eq!(in_filtered_child(&default_program(), probe), 0);
        // And the parent, which never installed a filter, is not filtered.
        assert_eq!(seccomp_mode(), Some(0));
    }

    #[test]
    fn bridge_filter_allows_unix_stream_but_nothing_more() {
        fn unix_stream() -> i32 {
            code(probe_socket(AF_UNIX, SOCK_STREAM | SOCK_CLOEXEC))
        }
        fn unix_dgram() -> i32 {
            code(probe_socket(AF_UNIX, SOCK_DGRAM))
        }
        fn netlink() -> i32 {
            code(probe_socket(AF_NETLINK, SOCK_RAW))
        }
        let program = default_policy().for_bridge().program(Gate::Silent).unwrap();
        assert_eq!(in_filtered_child(&program, unix_stream), 0);
        assert_eq!(in_filtered_child(&program, unix_dgram), EPERM);
        assert_eq!(in_filtered_child(&program, netlink), EPERM);
    }

    #[test]
    fn status_field_parses_the_proc_format() {
        // The running test process has both lines.
        assert_eq!(seccomp_mode(), Some(0));
        assert!(matches!(no_new_privs(), Some(0 | 1)));
        assert_eq!(status_field(b"\nNoSuchField:"), None);
    }
}
