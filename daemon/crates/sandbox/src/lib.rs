//! Sandbox-Backends, Profile und Isolations-Prüfungen.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! In Sprint 0 steht hier das Format, nicht der Start (ADR-002): ein Profil wird
//! gelesen, geprüft und in die vollständige `bwrap`-Kommandozeile übersetzt.
//! Ausgeführt wird sie erst von HUM-011, der Filter des Shims kommt mit HUM-012.
//! Damit ist die Politik der Sandbox eine Datei, die man lesen kann, und die
//! Zeile darunter ist ihre einzige Übersetzung.
//!
//! Aufbau:
//!
//! - [`profile`] die Typen des Profils, das Laden und die Mount-Allowlist
//! - [`bwrap_args`] die Übersetzung in die Argumentliste
//!
//! Wer ein Profil startet und nicht nur anzeigt, lädt es mit
//! [`SandboxProfile::load_validated`] gegen eine [`MountPolicy`], die der
//! Launcher (HUM-011) mit [`MountPolicy::from_paths`] aus
//! `humanitl_config::Paths` baut; nur so sind `$XDG_RUNTIME_DIR`,
//! `$XDG_CONFIG_HOME/humanitl` und `$XDG_DATA_HOME/humanitl` auch außerhalb von
//! `/run` und `$HOME` geschützt.
//!
//! ```
//! use std::path::Path;
//! use humanitl_sandbox::{MountPolicy, SandboxProfile, SessionContext};
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
//!     shim_src: "/usr/lib/humanitl/humanitl-shim".into(),
//!     command: vec!["opencode".into()],
//! };
//!
//! let argv = profile.to_bwrap_args(&ctx);
//! assert_eq!(argv[0], "--unshare-user");
//! assert!(profile.argv_line(&ctx).contains("--unshare-net"));
//! # Ok::<(), humanitl_core::Diagnostic>(())
//! ```
#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod bwrap_args;
pub mod profile;

pub use crate::profile::{
    Bridge, BridgeDirection, CA_CERT_DST, DEFAULT_DENY_SYSCALLS, FORBIDDEN_IN_HOME,
    FORBIDDEN_MOUNTS, HOSTNAME, MANDATORY_MASKED_FILES, MountPolicy, MountRule, MountSection,
    Namespace, NetworkSection, PROFILE_VERSION, PROXY_BRIDGE, PROXY_PORT, PROXY_SOCKET_DST,
    REQUIRED_TMPFS, SHIM_DST, SOCKET_WALK_MAX_DEPTH, SOCKET_WALK_MAX_ENTRIES, SandboxProfile,
    SandboxSection, SeccompSection, SessionContext, SocketFamily, SocketType, Symlink, WORK_DST,
    WorkMount,
};
