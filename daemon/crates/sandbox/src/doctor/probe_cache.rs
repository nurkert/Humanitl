//! Der Zwischenspeicher des Probestarts (HUM-436).
//!
//! Ein Probestart dauert Sekunden: Er startet eine Sandbox und wartet auf die
//! fünf `CHECK`-Zeilen des Shims. Sein Ergebnis hängt an den Werkzeugen, die
//! dabei liefen — dem Programm des Backends und seiner Fassung, und Humanitl
//! selbst, das den Shim und den Plan mitbringt. Solange keine dieser Fassungen
//! sich ändert, gilt ein gelungener Probestart weiter, und `humanitl doctor`
//! zeigt ihn, ohne erneut etwas zu starten.
//!
//! # Was der Schlüssel enthält
//!
//! Stufe, Programm, Fassung des Programms, Fassung von Humanitl und die
//! Kennung des Rechners, alle fünf in [`ProbeKey::id`]. Die Kennung des
//! Rechners steht darin, weil ein Datenverzeichnis zwischen Rechnern
//! synchronisiert sein kann: Ein Probestart gilt nur für die Maschine, auf
//! der er lief ([`machine_identity`]). Ein Eintrag gilt nur bei genau gleichem Schlüssel; ein
//! Update des Programms, etwa von bwrap, macht ihn ungültig, ohne dass jemand ihn
//! löschen muss.
//!
//! # Nur gelungene Probestarts
//!
//! Gespeichert wird nur ein Probestart, der bestanden hat. Ein gescheiterter
//! läuft beim nächsten `--probe` wieder: Wer einen Kernel-Schalter umgelegt
//! oder eine Gruppe delegiert hat, ändert dabei keine Fassung und soll trotzdem
//! sofort sehen, ob es geholfen hat.
//!
//! Die Datei ist ein Hinweis und keine Zusage: Was sich nicht lesen lässt,
//! zählt als leer, und was sich nicht schreiben lässt, kostet nur den nächsten
//! Probestart.

use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Der Name der Datei im Datenverzeichnis.
const FILE_NAME: &str = "doctor-probe.json";

/// Wonach ein Probestart gespeichert wird.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeKey {
    /// Die Stufe, zum Beispiel `bwrap`, mit Laufzeit, falls das Backend eine hat.
    pub stage: String,
    /// Das Programm des Backends, wie es gefunden wurde.
    pub program: String,
    /// Die Fassung dieses Programms; leer, wo das Backend keine nennt.
    pub tool_version: String,
    /// Die Fassung von Humanitl, die Shim und Plan mitbringt.
    pub humanitl_version: String,
    /// Die Kennung des Rechners ([`machine_identity`]).
    pub machine: String,
}

impl ProbeKey {
    /// Der Schlüssel als eine Zeile; gleich genau dann, wenn alle fünf Teile
    /// gleich sind.
    #[must_use]
    pub fn id(&self) -> String {
        format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.stage, self.program, self.tool_version, self.humanitl_version, self.machine
        )
    }
}

/// Die Kennung dieses Rechners für den Schlüssel: unter Linux
/// `/etc/machine-id` (sonst `/var/lib/dbus/machine-id`, sonst der
/// Hostname), unter macOS die UUID des Rechners (`kern.uuid`). Leer, wenn
/// nichts davon lesbar ist; dann gilt kein Eintrag ([`ProbeCache::lookup`]).
#[must_use]
pub fn machine_identity() -> String {
    #[cfg(target_os = "linux")]
    {
        [
            "/etc/machine-id",
            "/var/lib/dbus/machine-id",
            "/proc/sys/kernel/hostname",
        ]
        .iter()
        .find_map(|path| {
            std::fs::read_to_string(path)
                .ok()
                .map(|text| text.trim().to_owned())
                .filter(|text| !text.is_empty())
        })
        .unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    {
        std::process::Command::new("/usr/sbin/sysctl")
            .args(["-n", "kern.uuid"])
            .env_clear()
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
            .unwrap_or_default()
    }
}

/// Ein Eintrag der Datei.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entry {
    /// Die Stufe; je Stufe steht höchstens ein Eintrag in der Datei.
    stage: String,
    /// [`ProbeKey::id`] des Probestarts.
    key: String,
    /// Der Beleg des gelungenen Probestarts.
    evidence: String,
}

/// Der Inhalt der Datei.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Contents {
    entries: Vec<Entry>,
}

/// Der Zwischenspeicher, eine JSON-Datei.
#[derive(Debug, Clone)]
pub struct ProbeCache {
    path: PathBuf,
}

impl ProbeCache {
    /// Der Zwischenspeicher in dieser Datei.
    #[must_use]
    pub fn at(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Der Zwischenspeicher im Datenverzeichnis dieser Pfade.
    #[must_use]
    pub fn of(paths: &humanitl_config::Paths) -> Self {
        Self::at(paths.data_dir().join(FILE_NAME))
    }

    /// Die Datei.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Der Beleg eines gelungenen Probestarts mit genau diesem Schlüssel.
    #[must_use]
    pub fn lookup(&self, key: &ProbeKey) -> Option<String> {
        // Ohne Kennung des Rechners kann der Eintrag von einem anderen
        // stammen; dann gilt er nicht.
        if key.machine.is_empty() {
            return None;
        }
        let id = key.id();
        self.read()
            .entries
            .into_iter()
            .find(|entry| entry.key == id)
            .map(|entry| entry.evidence)
    }

    /// Merkt sich einen gelungenen Probestart; ein älterer Eintrag derselben
    /// Stufe weicht.
    ///
    /// # Errors
    ///
    /// Der Fehler des Dateisystems; der Aufrufer darf ihn übergehen, er kostet
    /// nur den nächsten Probestart.
    pub fn store(&self, key: &ProbeKey, evidence: &str) -> std::io::Result<()> {
        let mut contents = self.read();
        contents.entries.retain(|entry| entry.stage != key.stage);
        contents.entries.push(Entry {
            stage: key.stage.clone(),
            key: key.id(),
            evidence: evidence.to_owned(),
        });
        let text = serde_json::to_vec_pretty(&contents).map_err(std::io::Error::other)?;
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Erst daneben schreiben, dann umbenennen: Ein zweiter Doctor, der
        // gleichzeitig liest, sieht die alte oder die neue Datei, nie eine
        // halbe. Die Zwischendatei entsteht neu und exklusiv mit `0600`; eine
        // gleichnamige, die jemand mit anderen Rechten hingelegt hat, wird
        // nie weiterbenutzt, sondern der Name gewechselt.
        let mut attempt = 0_u32;
        let (partial, mut file) = loop {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |elapsed| elapsed.subsec_nanos());
            let partial = self
                .path
                .with_extension(format!("json.{}.{nanos}.{attempt}", std::process::id()));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&partial)
            {
                Ok(file) => break (partial, file),
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists && attempt < 8 => {
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        };
        let written = file
            .write_all(&text)
            .and_then(|()| file.sync_all())
            .and_then(|()| std::fs::rename(&partial, &self.path));
        if written.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        written
    }

    /// Der Inhalt; leer, wenn die Datei fehlt oder sich nicht lesen lässt.
    fn read(&self) -> Contents {
        std::fs::read(&self.path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::{ProbeCache, ProbeKey};

    fn key(tool: &str) -> ProbeKey {
        ProbeKey {
            stage: "engine/plain".to_owned(),
            program: "/usr/bin/engine".to_owned(),
            tool_version: tool.to_owned(),
            humanitl_version: "0.1.0".to_owned(),
            machine: "machine-a".to_owned(),
        }
    }

    /// Eine andere Fassung des Programms, etwa nach einem Update der
    /// Container-Engine, macht den gespeicherten Probestart ungültig.
    ///
    /// Mutationsbeweis: Ohne die Fassung in [`ProbeKey::id`] fände der
    /// Zwischenspeicher den Eintrag von 5.0.0 auch für 5.1.0, und dieser Test
    /// fiele.
    #[test]
    fn another_tool_version_invalidates_the_cache() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ProbeCache::at(dir.path().join("data/doctor-probe.json"));
        cache.store(&key("5.0.0"), "passed").unwrap();
        assert_eq!(cache.lookup(&key("5.0.0")).as_deref(), Some("passed"));
        assert_eq!(
            cache.lookup(&key("5.1.0")),
            None,
            "a new version probes again"
        );
    }

    #[test]
    fn another_humanitl_or_program_invalidates_it_too() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        cache.store(&key("5.0.0"), "passed").unwrap();
        let mut other = key("5.0.0");
        other.humanitl_version = "0.2.0".to_owned();
        assert_eq!(cache.lookup(&other), None);
        let mut other = key("5.0.0");
        other.program = "/opt/engine/bin/engine".to_owned();
        assert_eq!(cache.lookup(&other), None);
    }

    #[test]
    fn a_new_probe_of_a_stage_replaces_the_old_one_and_keeps_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        let bwrap = ProbeKey {
            stage: "bwrap".to_owned(),
            program: "/usr/bin/bwrap".to_owned(),
            tool_version: "0.11.0".to_owned(),
            humanitl_version: "0.1.0".to_owned(),
            machine: "machine-a".to_owned(),
        };
        cache.store(&bwrap, "bwrap passed").unwrap();
        cache.store(&key("5.0.0"), "old").unwrap();
        cache.store(&key("5.1.0"), "new").unwrap();
        assert_eq!(cache.lookup(&key("5.0.0")), None);
        assert_eq!(cache.lookup(&key("5.1.0")).as_deref(), Some("new"));
        assert_eq!(cache.lookup(&bwrap).as_deref(), Some("bwrap passed"));
    }

    /// Ein synchronisiertes Datenverzeichnis bringt den Probestart eines
    /// anderen Rechners mit; er zählt hier nicht.
    #[test]
    fn a_probe_of_another_machine_does_not_count_here() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        cache.store(&key("5.0.0"), "passed").unwrap();
        let mut here = key("5.0.0");
        here.machine = "machine-b".to_owned();
        assert_eq!(cache.lookup(&here), None);
    }

    /// Ohne Kennung des Rechners gilt kein Eintrag, auch keiner, der selbst
    /// ohne Kennung geschrieben wurde.
    #[test]
    fn without_a_machine_identity_nothing_counts() {
        let dir = tempfile::tempdir().unwrap();
        let cache = ProbeCache::at(dir.path().join("doctor-probe.json"));
        let mut nameless = key("5.0.0");
        nameless.machine = String::new();
        cache.store(&nameless, "passed").unwrap();
        assert_eq!(cache.lookup(&nameless), None);
    }

    /// Die Zwischendatei entsteht neu: Eine liegengebliebene mit weiten
    /// Rechten unter dem alten Namen wird weder geöffnet noch geändert, und
    /// die Datei selbst hat `0600`.
    #[test]
    fn the_staging_file_is_created_exclusively_with_0600() {
        use std::os::unix::fs::PermissionsExt as _;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doctor-probe.json");
        let stale = dir
            .path()
            .join(format!("doctor-probe.json.{}", std::process::id()));
        std::fs::write(&stale, b"stale").unwrap();
        std::fs::set_permissions(&stale, std::fs::Permissions::from_mode(0o666)).unwrap();
        let cache = ProbeCache::at(&path);
        cache.store(&key("5.0.0"), "passed").unwrap();
        assert_eq!(std::fs::read(&stale).unwrap(), b"stale");
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let left: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .map(|entry| entry.file_name())
            .collect();
        assert_eq!(left.len(), 2, "no staging file stays behind: {left:?}");
    }

    #[test]
    fn an_unreadable_file_is_an_empty_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doctor-probe.json");
        std::fs::write(&path, b"{ not json").unwrap();
        let cache = ProbeCache::at(&path);
        assert_eq!(cache.lookup(&key("5.0.0")), None);
        cache.store(&key("5.0.0"), "passed").unwrap();
        assert_eq!(cache.lookup(&key("5.0.0")).as_deref(), Some("passed"));
    }
}
