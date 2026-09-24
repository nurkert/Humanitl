//! Der Session-Socket: genau eine Tür in die Sandbox (Garantie 2).
//!
//! Eine Unix-Socket-Datei in einem eigenen Verzeichnis
//! (`Paths::proxy_socket()`, also `$XDG_RUNTIME_DIR/humanitl/proxy/proxy.sock`;
//! Verzeichnis `0700`, Datei `0600`). Der Launcher hängt genau diese Datei als
//! `/run/humanitl/proxy.sock` in die Sandbox; der gRPC-Socket und die
//! Token-Datei bleiben unsichtbar, weil sie in einem anderen Verzeichnis
//! liegen (HUM-013, Security-Review Punkt 3).
//!
//! Beim Fallenlassen verschwindet die Datei. Der Socket wird einmal erzeugt und
//! nie während der Sitzung neu angelegt, damit der in die Sandbox eingehängte
//! Inode derselbe bleibt (Fallstrick HUM-013).

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};

use humanitl_config::private_dir::{ensure_private_dir, process_uid};
use humanitl_core::diagnostics::codes::DAEMON_004;
use humanitl_core::{Diagnostic, FixAction, SessionId, Severity};
use tokio::net::UnixListener;

/// Rechte der Socket-Datei.
pub const SOCKET_MODE: u32 = 0o600;
/// Obergrenze für einen Unix-Socket-Pfad (`sun_path`, inklusive Nullbyte).
pub const SUN_PATH_MAX: usize = 108;

/// Der Socket einer Sitzung. Lauscht und räumt sich beim Fallenlassen auf.
#[derive(Debug)]
pub struct SessionSocket {
    path: PathBuf,
    listener: UnixListener,
}

impl SessionSocket {
    /// Bindet den Socket genau unter `path` (etwa `Paths::proxy_socket()`).
    ///
    /// Legt das Elternverzeichnis mit `0700` an, entfernt eine gleichnamige
    /// Altdatei (verwaister Socket eines abgestürzten Laufs), bindet und setzt
    /// die Rechte der Datei auf `0600`. Der Eigentümer ist die laufende UID.
    ///
    /// # Errors
    ///
    /// [`DAEMON_004`], wenn der Pfad zu lang für `sun_path` ist, das
    /// Verzeichnis nicht anlegbar oder der Bind nicht möglich ist.
    pub fn bind(path: &Path) -> Result<Self, Diagnostic> {
        if path.as_os_str().len() >= SUN_PATH_MAX {
            return Err(Diagnostic::builder(DAEMON_004, Severity::Error)
                .why(format!(
                    "socket path {} is {} bytes, over the {} byte sun_path limit",
                    path.display(),
                    path.as_os_str().len(),
                    SUN_PATH_MAX
                ))
                .fix(FixAction::SetEnv {
                    key: "XDG_RUNTIME_DIR".to_owned(),
                    value: "/run/user/<uid>".to_owned(),
                })
                .build());
        }

        if let Some(dir) = path.parent() {
            Self::ensure_dir(dir)?;
        }
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path).map_err(|err| {
            Diagnostic::builder(DAEMON_004, Severity::Error)
                .why(format!("cannot bind {}: {err}", path.display()))
                .build()
        })?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(SOCKET_MODE)).map_err(
            |err| {
                let _ = std::fs::remove_file(path);
                Diagnostic::builder(DAEMON_004, Severity::Error)
                    .why(format!("cannot chmod {} to 0600: {err}", path.display()))
                    .build()
            },
        )?;
        Ok(Self {
            path: path.to_owned(),
            listener,
        })
    }

    /// Wie [`SessionSocket::bind`], mit dem Pfad `dir/<session>.sock`, für
    /// einen Daemon mit mehreren Sitzungen nebeneinander.
    ///
    /// # Errors
    ///
    /// Wie [`SessionSocket::bind`].
    pub fn create(dir: &Path, session: SessionId) -> Result<Self, Diagnostic> {
        Self::bind(&dir.join(format!("{session}.sock")))
    }

    /// Der Pfad der Socket-Datei.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Der lauschende Listener.
    #[must_use]
    pub const fn listener(&self) -> &UnixListener {
        &self.listener
    }

    /// Legt das Socket-Verzeichnis an wie der Daemon sein Laufzeitverzeichnis:
    /// kein Symlink, eigener Besitzer, danach `0700` (HUM-212).
    fn ensure_dir(dir: &Path) -> Result<(), Diagnostic> {
        ensure_private_dir(dir, process_uid())
    }
}

impl Drop for SessionSocket {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::os::unix::fs::PermissionsExt as _;

    use humanitl_core::diagnostics::codes::DAEMON_004;

    use super::SessionSocket;

    /// Ein Symlink an der Stelle des Socket-Verzeichnisses wird abgewiesen,
    /// nicht verfolgt: sonst läge der Socket in einem Verzeichnis, das ein
    /// anderes Konto ausgesucht hat (HUM-212).
    #[tokio::test]
    async fn a_symlink_in_place_of_the_socket_dir_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let target = tmp.path().join("elsewhere");
        std::fs::create_dir(&target).unwrap();
        std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o755)).unwrap();
        let dir = tmp.path().join("proxy");
        std::os::unix::fs::symlink(&target, &dir).unwrap();

        let error = SessionSocket::bind(&dir.join("proxy.sock")).unwrap_err();

        assert_eq!(error.code, DAEMON_004);
        assert!(error.why.contains("symlink"), "{}", error.why);
        assert!(!target.join("proxy.sock").exists());
    }
}
