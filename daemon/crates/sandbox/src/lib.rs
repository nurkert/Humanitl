//! Sandbox-Backends, Profile und Isolations-Prüfungen.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! Die Politik der Sandbox ist eine Datei, die man lesen kann (ADR-002): ein
//! Profil wird gelesen, geprüft und in die vollständige `bwrap`-Kommandozeile
//! übersetzt; die Zeile darunter ist ihre einzige Übersetzung, und was die
//! Oberfläche unter „Sandbox" zeigt, ist Argument für Argument das, was
//! startet.
//!
//! Aufbau:
//!
//! - [`profile`] die Typen des Profils, das Laden und die Mount-Allowlist
//!   (HUM-010)
//! - [`agent`] der Port [`AgentAdapter`] mit [`AgentContext`],
//!   [`SandboxFile`] und [`AdapterRegistry`]; der einzige Adapter des MVP ist
//!   [`OpenCodeAdapter`] (HUM-037)
//! - [`bwrap_args`] die Übersetzung in die Argumentliste
//! - [`bridge_env`] der Vertrag zwischen Launcher und Shim: Umgebung,
//!   Bericht, Exit-Codes (HUM-011, HUM-012, HUM-013)
//! - [`launcher`] der Port [`SandboxBackend`] mit [`LaunchPlan`],
//!   [`CheckResult`] und [`IsolationCheck`]
//! - [`bwrap`] das Backend [`BwrapBackend`]: findet `bwrap`, prüft, plant,
//!   startet mit leerer Umgebung
//! - [`handle`] die laufende Sandbox: [`SandboxHandle`] mit `wait`, `kill`
//!   und dem Bericht des Shims
//! - [`worktree`] der Blick des Hosts in `/work`: Schnappschuss, Diff,
//!   Symlink-Erkennung und das sichere Öffnen mit `openat2` (HUM-043)
//! - [`summary`] was ein Sandbox-Lauf im Projekt hinterlassen hat, in der Form,
//!   die ein Mensch zu sehen bekommt (HUM-043)
//!
//! Wer ein Profil startet und nicht nur anzeigt, lädt es mit
//! [`SandboxProfile::load_validated`] gegen eine [`MountPolicy`] aus
//! [`MountPolicy::from_paths`] und `humanitl_config::Paths`; nur so sind
//! `$XDG_RUNTIME_DIR`, `$XDG_CONFIG_HOME/humanitl` und
//! `$XDG_DATA_HOME/humanitl` auch außerhalb von `/run` und `$HOME`
//! geschützt. Dieselben `Paths` bekommt das Backend
//! ([`BwrapBackend::detect`]); [`SandboxBackend::plan`] prüft damit auch das
//! Projektverzeichnis und den Proxy-Socket.
//!
//! ```
//! use std::path::Path;
//! use humanitl_sandbox::{LaunchInputs, MountPolicy, SandboxProfile, SessionContext};
//! use humanitl_config::{Env, WorkMode};
//! use humanitl_core::ids::SessionId;
//!
//! let profile = SandboxProfile::parse(
//!     "version = 1\nname = \"demo\"\n",
//!     Path::new("<doc>"),
//! )?;
//! let policy = MountPolicy::from_env(&Env::from_pairs([("HOME", "/home/u")]));
//! profile.validate_with(&policy)?;
//! let ctx = SessionContext {
//!     session: SessionId::nil(),
//!     work_src: "/home/u/proj".into(),
//!     work_mode: WorkMode::Rw,
//!     proxy_socket_src: "/run/user/1000/humanitl/proxy/proxy.sock".into(),
//!     ca_cert_src: "/home/u/.local/share/humanitl/ca/ca.crt".into(),
//!     ca_bundle_src: "/home/u/.local/share/humanitl/ca/ca-bundle.crt".into(),
//!     shim_src: "/usr/lib/humanitl/humanitl-shim".into(),
//!     // Aus `humanitl_proxy::ca::env_kit(session)`; das Profil kennt die
//!     // Sitzung nicht.
//!     session_env: vec![("HUMANITL_SESSION".to_owned(), SessionId::nil().to_string())],
//!     command: vec!["opencode".into()],
//!     files: Vec::new(),
//! };
//!
//! // Die Vorschau: feste Deskriptornummern, alles unter /work als vorhanden.
//! let argv = profile.to_bwrap_args(&ctx, &LaunchInputs::preview());
//! assert_eq!(argv[0], "--unshare-user");
//! assert!(profile.argv_line(&ctx, &LaunchInputs::preview()).contains("--unshare-net"));
//! # Ok::<(), humanitl_core::Diagnostic>(())
//! ```
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod agent;
pub mod bridge_env;
pub mod bwrap;
pub mod bwrap_args;
pub mod handle;
pub mod launcher;
pub mod profile;
pub mod summary;
pub mod worktree;

pub use crate::agent::{
    AdapterRegistry, AgentAdapter, AgentContext, OpenCodeAdapter, SandboxFile, files_inside_work,
    find_in_path,
};
pub use crate::bridge_env::{
    CHECK_BRIDGE_LISTENING, CHECK_FAMILIES, CHECK_NAMES, CHECK_NO_INTERFACES, CHECK_PREFIX,
    CHECK_SECCOMP_APPLIED, CHECK_SINGLE_SOCKET, ENV_BRIDGES, ENV_REPORT_FD, ENV_SECCOMP_DENY,
    ENV_SECCOMP_FAMILIES, ENV_SECCOMP_TYPES, EXIT_EXEC, EXIT_SETUP, EXIT_USAGE, RESERVED_ENV,
    ShimCheck, bridges_json, parse_check_line, shim_env,
};
pub use crate::bwrap::{
    BwrapBackend, EARLY_EXIT_WINDOW, INSTALL_COMMAND, MIN_BWRAP_VERSION, REPORT_TIMEOUT,
    USERNS_DOCS_URL, USERNS_SYSCTL_COMMAND, Version, is_userns_failure,
};
pub use crate::bwrap_args::{
    DEFAULT_HOME, DEFAULT_USER, GROUP_DST, HOSTS_DST, IdentityFds, IdentityFiles, LaunchInputs,
    MaskFds, PASSWD_DST, PREVIEW_AGENT_FD_FIRST, PREVIEW_MASK_FD_FIRST, SANDBOX_SHELL, shell_line,
    shell_quote,
};
pub use crate::handle::{
    CAPTURE_MAX_BYTES, CapturedOutput, INTERRUPT_GRACE, KILL_GRACE, OutputChunk, OutputSink,
    OutputStream, ReportSnapshot, STATUS_DRAIN, STDERR_EXCERPT_BYTES, SandboxHandle,
    StatusSnapshot,
};
pub use crate::launcher::{CheckResult, IsolationCheck, LaunchPlan, SandboxBackend, StdioMode};
pub use crate::profile::{
    Bridge, BridgeDirection, CA_BUNDLE_DST, CA_CERT_DST, DEFAULT_DENY_SYSCALLS, FORBIDDEN_IN_HOME,
    FORBIDDEN_MOUNTS, HOSTNAME, MANDATORY_MASKED_FILES, MountPolicy, MountRule, MountSection,
    Namespace, NetworkSection, PROFILE_VERSION, PROXY_BRIDGE, PROXY_PORT, PROXY_SOCKET_DST,
    REQUIRED_SOCKET_FAMILIES, REQUIRED_SOCKET_TYPES, REQUIRED_TMPFS, SHIM_DST,
    SOCKET_WALK_MAX_DEPTH, SOCKET_WALK_MAX_ENTRIES, SandboxProfile, SandboxSection, SeccompSection,
    SessionContext, SocketFamily, SocketFloor, SocketType, Symlink, WORK_DST, WorkMount,
    is_mandatory_mask,
};
pub use crate::summary::{
    ChangeKind, FileChangeRecord, SCAN_MAX_BYTES, SessionSummary, SummaryFinding, SymlinkEscape,
    executable_on_host, looks_like_text,
};
pub use crate::worktree::{
    Entry, FileChange, Kind, Resolution, SnapshotLimits, TreeSnapshot, diff, escapes, open_beneath,
    open_root, read_beneath, snapshot,
};
