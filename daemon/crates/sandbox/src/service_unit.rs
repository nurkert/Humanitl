//! Der Weg zur heutigen Unit des Daemons, je nach Art der Installation
//! (HUM-529).
//!
//! Bis Humanitl 0.4.0 trug die Unit `ProtectKernelModules=yes`, das
//! `/usr/lib/modules` im Mount-Namensraum des Daemons verbarg. Ein Befund,
//! der darauf zurückgeht (`SANDBOX_045`, `SANDBOX_049`), schlägt den Weg zur
//! heutigen Unit vor, und der hängt davon ab, woher sie kommt: Das Paket legt
//! sie unter `/usr/lib/systemd/user` ab, und nach dessen Update lädt
//! `systemctl --user daemon-reload` sie neu; ohne Paket (Archiv, `AppImage`)
//! schreibt `humanitl daemon install` sie. Dieselbe Regel wie der Vorschlag
//! für „kein Daemon" in `humanitl_ipc::auth::no_daemon_fix`; die Sandbox
//! hängt nicht von `ipc` ab und liest die Lage deshalb selbst.

use std::path::{Path, PathBuf};

use humanitl_config::Paths;
use humanitl_core::FixAction;

/// Wo das Paket die Units der Nutzer-Ebene ablegt.
const PACKAGE_UNIT_DIR: &str = "/usr/lib/systemd/user";

/// Die Variable, die den Ort für Tests ersetzt (wie in `ipc::auth`).
const PACKAGE_UNIT_DIR_ENV: &str = "HUMANITL_SYSTEM_UNIT_DIR";

/// Die Variable eines Laufs aus einem `AppImage`.
const APPIMAGE_ENV: &str = "APPIMAGE";

/// Die Dienst-Unit.
const SERVICE_UNIT: &str = "humanitld.service";

/// Nach dem Update des Pakets: die neue Unit laden und den Dienst neu starten.
pub(crate) const RESTART_COMMAND: &str =
    "systemctl --user daemon-reload && systemctl --user restart humanitld.service";

/// Beide Wege in einem Satz, für das `why` eines Befunds.
pub(crate) const BOTH_WAYS: &str = "with the package, update it and run `systemctl --user \
     daemon-reload && systemctl --user restart humanitld.service`; without the package \
     (archive, AppImage), run `humanitl daemon install`";

/// Woher die Unit des Daemons kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InstallKind {
    /// Das Paket hat `humanitld.service` abgelegt, und der Lauf kommt nicht
    /// aus einem `AppImage`.
    package: bool,
    /// `humanitl daemon install` hat eine Unit des Nutzers geschrieben; sie
    /// verdeckt die des Pakets.
    user_unit: bool,
}

impl InstallKind {
    /// Liest die Lage für diese Pfade.
    pub(crate) fn probe(paths: &Paths) -> Self {
        let package_dir = paths
            .env()
            .non_empty(PACKAGE_UNIT_DIR_ENV)
            .map_or_else(|| PathBuf::from(PACKAGE_UNIT_DIR), PathBuf::from);
        let user_dir = paths
            .config_dir()
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
            .join("systemd")
            .join("user");
        Self {
            package: paths.env().non_empty(APPIMAGE_ENV).is_none()
                && package_dir.join(SERVICE_UNIT).is_file(),
            user_unit: user_dir.join(SERVICE_UNIT).is_file(),
        }
    }

    /// Der Weg zur heutigen Unit: Mit dem Paket allein [`RESTART_COMMAND`]
    /// nach dessen Update; liegt eine Unit des Nutzers da (sie verdeckt die
    /// des Pakets) oder gibt es kein Paket, `humanitl daemon install`.
    pub(crate) fn fix(self) -> FixAction {
        if self.package && !self.user_unit {
            FixAction::CopyCommand(RESTART_COMMAND.to_owned())
        } else {
            FixAction::InstallService
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_config::Env;

    use super::*;

    fn paths(dir: &Path, appimage: bool) -> Paths {
        let mut pairs = vec![
            ("HOME", dir.join("home").display().to_string()),
            ("XDG_CONFIG_HOME", dir.join("config").display().to_string()),
            (
                PACKAGE_UNIT_DIR_ENV,
                dir.join("package").display().to_string(),
            ),
        ];
        if appimage {
            pairs.push((APPIMAGE_ENV, "/tmp/x.AppImage".to_owned()));
        }
        Paths::new(Env::from_pairs(pairs))
    }

    /// Paket allein: neu laden und neu starten. Ohne Paket, mit einer Unit
    /// des Nutzers daneben oder aus einem `AppImage`: `daemon install`.
    #[test]
    fn the_way_to_the_current_unit_follows_the_install_kind() {
        let dir = tempfile::tempdir().unwrap();
        let none = InstallKind::probe(&paths(dir.path(), false));
        assert_eq!(none.fix(), FixAction::InstallService);

        std::fs::create_dir_all(dir.path().join("package")).unwrap();
        std::fs::write(dir.path().join("package").join(SERVICE_UNIT), "").unwrap();
        let package = InstallKind::probe(&paths(dir.path(), false));
        assert_eq!(
            package.fix(),
            FixAction::CopyCommand(RESTART_COMMAND.to_owned())
        );
        let appimage = InstallKind::probe(&paths(dir.path(), true));
        assert_eq!(appimage.fix(), FixAction::InstallService);

        let user = dir.path().join("config/systemd/user");
        std::fs::create_dir_all(&user).unwrap();
        std::fs::write(user.join(SERVICE_UNIT), "").unwrap();
        let both = InstallKind::probe(&paths(dir.path(), false));
        assert_eq!(both.fix(), FixAction::InstallService);
    }
}
