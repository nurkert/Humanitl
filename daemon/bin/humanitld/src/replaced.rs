//! Ein Daemon, dessen Programm ein Paket-Update ersetzt hat, geht, sobald
//! keine Sitzung mehr läuft (HUM-284).
//!
//! `dpkg` legt ein neues `humanitld` neben das alte und benennt es dann über
//! dessen Pfad. Der laufende Prozess behält das alte Programm, und
//! `/proc/self/exe` zeigt danach auf `…/humanitld (deleted)`, und unter dem
//! Pfad ohne das Anhängsel liegt eine andere Datei. Beides zusammen ist die
//! Marke: Sie steht da, ohne dass `postinst` etwas schreiben müsste.
//!
//! **Ersetzt, nicht entfernt.** Nach `apt remove` steht dasselbe Anhängsel
//! da, aber unter dem Pfad liegt nichts mehr; ein Ende brächte dann kein
//! neues Programm, sondern gar keins. Der Daemon bleibt in diesem Fall, bis
//! sein Mensch ihn anhält.
//!
//! `postinst` bittet einen Daemon über `humanitl daemon retire-if-idle`, sich
//! zu verabschieden, und startet ihn danach neu. Hält ihn gerade eine Sitzung,
//! bleibt er stehen; dann sieht dieser Wächter alle [`POLL`] nach, und
//! sobald [`SandboxService::retire_if_idle`] keinen Anspruch und kein Handle
//! mehr findet, endet der Dienst mit dem gewöhnlichen Abschied. Die
//! Socket-Unit hält den Socket weiter, und die nächste Verbindung (die App
//! verbindet sich von selbst neu) startet das neue Programm. Bis dahin zeigt
//! die App den Hinweis auf die andere Version (HUM-277).
//!
//! Nur mit einem Socket von systemd: Ohne Socket-Aktivierung startete nach
//! dem Ende niemand das neue Programm, und ein Daemon, der von Hand lief,
//! wäre einfach fort.

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::MetadataExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use humanitl_ipc::SandboxService;

/// Wie oft der Wächter nachsieht.
///
/// Ein Blick ist ein `readlink` und ein Schloss; zehn Sekunden sind ein
/// Bruchteil der Zeit, die ein Mensch nach dem Ende einer Sitzung braucht,
/// um die nächste zu beginnen.
pub const POLL: Duration = Duration::from_secs(10);

/// Der Grund in `daemon.stopped`, wenn der Dienst sich ohne Sitzung
/// verabschiedet hat: auf `SandboxRequest.retire_if_idle` oder durch diesen
/// Wächter.
pub const REASON: &str = "retired";

/// Das Anhängsel, das der Kern an den Verweis einer gelöschten Datei hängt.
const DELETED: &[u8] = b" (deleted)";

/// Der Pfad, an dem das laufende Programm lag, wenn der Verweis `link` aus
/// `/proc/self/exe` sagt, dass es gelöscht ist; sonst `None`.
#[must_use]
pub fn deleted_path(link: &OsStr) -> Option<PathBuf> {
    link.as_bytes()
        .strip_suffix(DELETED)
        .filter(|path| !path.is_empty())
        .map(|path| PathBuf::from(OsStr::from_bytes(path)))
}

/// Ob unter `path` ein anderes Programm liegt als `running` (Gerät und
/// Inode des laufenden). Fehlt die Datei, ist nichts ersetzt, sondern
/// entfernt.
#[must_use]
pub fn replaced_at(path: &Path, running: (u64, u64)) -> bool {
    std::fs::metadata(path).is_ok_and(|now| (now.dev(), now.ino()) != running)
}

/// Ob das Programm dieses Prozesses durch ein anderes ersetzt wurde.
///
/// Ein Verweis, der sich nicht lesen lässt, ein gelöschtes Programm ohne
/// Nachfolger und eines, dessen Pfad noch dieselbe Datei trägt, sind kein
/// ersetztes Programm: Der Wächter beendet nie einen Dienst auf eine
/// Vermutung hin.
#[must_use]
pub fn own_program_replaced() -> bool {
    let Ok(link) = std::fs::read_link("/proc/self/exe") else {
        return false;
    };
    let Some(path) = deleted_path(link.as_os_str()) else {
        return false;
    };
    // `metadata` folgt dem Verweis zur laufenden Datei, auch wenn sie
    // gelöscht ist.
    let Ok(running) = std::fs::metadata("/proc/self/exe") else {
        return false;
    };
    replaced_at(&path, (running.dev(), running.ino()))
}

/// Wartet, bis das Programm ersetzt ist und keine Sitzung mehr läuft, und
/// gibt dann [`REASON`] zurück.
///
/// `replaced` ist die Frage nach dem Programm, `poll` der Abstand der Blicke;
/// beide nur für Tests veränderlich. Der erste Blick fällt nach `poll`, nicht
/// sofort.
pub async fn retire_when_replaced(
    sandbox: SandboxService,
    replaced: impl Fn() -> bool + Send + 'static,
    poll: Duration,
) -> &'static str {
    let mut announced = false;
    loop {
        tokio::time::sleep(poll).await;
        if !replaced() {
            continue;
        }
        if sandbox.retire_if_idle() {
            tracing::info!(
                "the program of this daemon was replaced and no session runs; ending so that \
                 socket activation starts the new one"
            );
            return REASON;
        }
        if !announced {
            tracing::info!(
                "the program of this daemon was replaced; it ends once the running session ends"
            );
            announced = true;
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::ffi::OsStr;
    use std::path::PathBuf;
    use std::time::Duration;

    use humanitl_config::Config;
    use humanitl_core::SessionId;
    use humanitl_ipc::SandboxService;

    use super::{REASON, deleted_path, replaced_at, retire_when_replaced};

    fn service() -> SandboxService {
        let paths =
            humanitl_config::Paths::new(humanitl_config::Env::from_pairs([("HOME", "/home/u")]));
        SandboxService::new(
            humanitl_ipc::session::SessionResolver::for_config(paths, Config::default()),
            SessionId::nil(),
            humanitl_ipc::sandbox::SandboxPorts::none(),
        )
    }

    /// Nur das Anhängsel des Kerns zählt, nicht ein Pfad, der „deleted"
    /// enthält.
    #[test]
    fn only_the_kernel_suffix_marks_a_deleted_program() {
        assert_eq!(
            deleted_path(OsStr::new("/usr/lib/humanitl/bin/humanitld (deleted)")),
            Some(PathBuf::from("/usr/lib/humanitl/bin/humanitld"))
        );
        assert_eq!(
            deleted_path(OsStr::new("/usr/lib/humanitl/bin/humanitld")),
            None
        );
        assert_eq!(deleted_path(OsStr::new("/opt/deleted/humanitld")), None);
        assert_eq!(deleted_path(OsStr::new("/opt/x/humanitld(deleted)")), None);
        assert_eq!(deleted_path(OsStr::new(" (deleted)")), None);
    }

    /// Ersetzt heißt: unter dem Pfad liegt eine andere Datei. Dieselbe Datei
    /// ist nichts, und eine fehlende Datei (nach `apt remove`) auch nicht.
    #[test]
    fn replaced_means_another_file_at_the_path_not_none() {
        use std::os::unix::fs::MetadataExt as _;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("humanitld");
        std::fs::write(&path, b"old").unwrap();
        let old = std::fs::metadata(&path).unwrap();
        let running = (old.dev(), old.ino());
        assert!(!replaced_at(&path, running), "still the running file");

        // Wie dpkg: die neue Datei daneben, dann über den Pfad benannt.
        let new = dir.path().join("humanitld.dpkg-new");
        std::fs::write(&new, b"new").unwrap();
        std::fs::rename(&new, &path).unwrap();
        assert!(replaced_at(&path, running), "another file at the path");

        std::fs::remove_file(&path).unwrap();
        assert!(!replaced_at(&path, running), "removed, not replaced");
    }

    /// Solange das Programm nicht ersetzt ist, geht der Daemon nie, auch ohne
    /// Sitzung.
    #[tokio::test(start_paused = true)]
    async fn an_unreplaced_daemon_stays() {
        let sandbox = service();
        let watcher = retire_when_replaced(sandbox.clone(), || false, Duration::from_secs(10));
        let ended = tokio::time::timeout(Duration::from_secs(3600), watcher).await;
        assert!(ended.is_err(), "an hour without a replaced program");
        assert!(!sandbox.is_leaving());
    }

    /// Ersetzt und ohne Sitzung: Der Wächter endet mit seinem Grund und hat
    /// den Riegel vorgeschoben.
    #[tokio::test(start_paused = true)]
    async fn a_replaced_daemon_without_a_session_ends() {
        let sandbox = service();
        let reason = tokio::time::timeout(
            Duration::from_secs(11),
            retire_when_replaced(sandbox.clone(), || true, Duration::from_secs(10)),
        )
        .await
        .expect("the first look ends it");
        assert_eq!(reason, REASON);
        assert!(sandbox.is_leaving(), "no start gets a claim afterwards");
    }
}
