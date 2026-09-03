//! Wo alles liegt. Nach der XDG-Spezifikation, ohne globalen Zustand.
//!
//! Die Pfade stehen in `backlog/CONVENTIONS.md` 3.4 und sind für Daemon,
//! Kommandozeile, Sandbox und Tests dieselben. [`Paths`] rechnet sie aus einer
//! übergebenen [`Env`] aus und liest dabei nichts aus der Umgebung des
//! Prozesses; nur [`Paths::from_process`] tut das, ein einziges Mal.
//!
//! Angelegt wird hier nichts. Wer ein Verzeichnis braucht, legt es an und
//! benutzt dafür [`DIR_MODE`] und [`FILE_MODE`]: das Laufzeitverzeichnis ist
//! `0700`, Socket und Token darin sind `0600`. Ein Socket, den die halbe
//! Maschine öffnen darf, wäre der bequemste Weg an jeder Entscheidung vorbei.

use std::path::{Path, PathBuf};

use humanitl_core::diagnostics::codes::CONFIG_004;
use humanitl_core::{Diagnostic, FixAction, Severity};

use crate::env::Env;

/// Der Name, unter dem Humanitl seine Verzeichnisse anlegt.
pub const APP_DIR: &str = "humanitl";

/// Rechte für Verzeichnisse, die nur den Nutzer angehen (`0700`).
pub const DIR_MODE: u32 = 0o700;

/// Rechte für Socket und Token (`0600`).
pub const FILE_MODE: u32 = 0o600;

/// Das Laufzeitverzeichnis samt Befund, falls es ein Ersatz ist.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeDir {
    /// Das Verzeichnis, in dem Socket und Token liegen.
    pub path: PathBuf,
    /// Gesetzt, wenn weder `XDG_RUNTIME_DIR` noch `/run/user/<uid>` taugte.
    pub diagnostic: Option<Diagnostic>,
}

/// Alle Pfade, abgeleitet aus einer Umgebung.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    env: Env,
}

impl Paths {
    /// Pfade aus einer übergebenen Umgebung.
    #[must_use]
    pub const fn new(env: Env) -> Self {
        Self { env }
    }

    /// Pfade aus der Umgebung des laufenden Prozesses.
    #[must_use]
    pub fn from_process() -> Self {
        Self::new(Env::from_process())
    }

    /// Die Umgebung, aus der die Pfade stammen.
    #[must_use]
    pub const fn env(&self) -> &Env {
        &self.env
    }

    /// Das Heimatverzeichnis. Ohne `HOME` bleibt nur ein Pfad unter `/tmp`.
    #[must_use]
    pub fn home(&self) -> PathBuf {
        self.env.non_empty("HOME").map_or_else(
            || {
                self.tmp_base()
                    .join(format!("{APP_DIR}-{}-home", self.env.uid()))
            },
            PathBuf::from,
        )
    }

    /// `$XDG_CONFIG_HOME/humanitl`, sonst `~/.config/humanitl`.
    #[must_use]
    pub fn config_dir(&self) -> PathBuf {
        self.xdg("XDG_CONFIG_HOME", ".config").join(APP_DIR)
    }

    /// `config.toml` im Konfigurationsverzeichnis.
    #[must_use]
    pub fn config_path(&self) -> PathBuf {
        self.config_dir().join("config.toml")
    }

    /// `rules.yaml` neben der `config.toml`.
    #[must_use]
    pub fn rules_path(&self) -> PathBuf {
        self.config_dir().join("rules.yaml")
    }

    /// Das Verzeichnis der Profile.
    #[must_use]
    pub fn profiles_dir(&self) -> PathBuf {
        self.config_dir().join("profiles")
    }

    /// Das Profil mit diesem Namen, ohne Endung angegeben.
    #[must_use]
    pub fn profile_path(&self, name: &str) -> PathBuf {
        self.profiles_dir().join(format!("{name}.toml"))
    }

    /// Das Profil eines Projekts: `<projekt>/.humanitl/profile.toml`.
    #[must_use]
    pub fn project_profile_path(&self, project: &Path) -> PathBuf {
        project.join(".humanitl").join("profile.toml")
    }

    /// `$XDG_DATA_HOME/humanitl`, sonst `~/.local/share/humanitl`.
    #[must_use]
    pub fn data_dir(&self) -> PathBuf {
        self.xdg_data().join(APP_DIR)
    }

    /// Die Datenbank der Aufzeichnung.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        self.data_dir().join("humanitl.db")
    }

    /// Das Verzeichnis der eigenen CA.
    #[must_use]
    pub fn ca_dir(&self) -> PathBuf {
        self.data_dir().join("ca")
    }

    /// Das Zertifikat der eigenen CA.
    #[must_use]
    pub fn ca_cert_path(&self) -> PathBuf {
        self.ca_dir().join("ca.crt")
    }

    /// Der Schlüssel der eigenen CA. Rechte `0600`.
    #[must_use]
    pub fn ca_key_path(&self) -> PathBuf {
        self.ca_dir().join("ca.key")
    }

    /// Das Wurzelverzeichnis der Blobs.
    #[must_use]
    pub fn blobs_dir(&self) -> PathBuf {
        self.data_dir().join("blobs")
    }

    /// Der Blob zu einer Prüfsumme: `blobs/<hex[0..2]>/<hex>`.
    ///
    /// Die beiden ersten Zeichen sind ein Fächer, damit kein Verzeichnis mit
    /// hunderttausend Einträgen entsteht.
    #[must_use]
    pub fn blob_path(&self, sha256_hex: &str) -> PathBuf {
        let shard = sha256_hex.get(0..2).unwrap_or(sha256_hex);
        self.blobs_dir().join(shard).join(sha256_hex)
    }

    /// Das Verzeichnis des Audit-Logs.
    #[must_use]
    pub fn audit_dir(&self) -> PathBuf {
        self.data_dir().join("audit")
    }

    /// Das Audit-Log selbst.
    #[must_use]
    pub fn audit_path(&self) -> PathBuf {
        self.audit_dir().join("audit.jsonl")
    }

    /// Das Laufzeitverzeichnis, mit Befund, falls es ein Ersatz ist.
    ///
    /// Reihenfolge: `$XDG_RUNTIME_DIR`, dann `/run/user/<uid>`, wenn es
    /// existiert, sonst `$TMPDIR/humanitl-<uid>` mit einem Hinweis
    /// (`CONFIG_004`, Stufe `info`). Der letzte Fall ist kein Fehler, aber er
    /// überlebt keinen Neustart und `/tmp` ist geteilt; wer ihn sieht, soll
    /// wissen, warum.
    #[must_use]
    pub fn runtime_dir(&self) -> RuntimeDir {
        self.runtime_dir_with(&|path| path.is_dir())
    }

    /// Wie [`Paths::runtime_dir`], aber mit einer eigenen Prüfung, ob ein
    /// Verzeichnis existiert. Für Tests, die kein `/run/user` haben.
    #[must_use]
    pub fn runtime_dir_with(&self, exists: &dyn Fn(&Path) -> bool) -> RuntimeDir {
        if let Some(dir) = self.env.non_empty("XDG_RUNTIME_DIR") {
            return RuntimeDir {
                path: Path::new(dir).join(APP_DIR),
                diagnostic: None,
            };
        }

        let uid = self.env.uid();
        let run_user = PathBuf::from(format!("/run/user/{uid}"));
        if exists(&run_user) {
            return RuntimeDir {
                path: run_user.join(APP_DIR),
                diagnostic: None,
            };
        }

        let path = self.tmp_base().join(format!("{APP_DIR}-{uid}"));
        let diagnostic = Diagnostic::builder(CONFIG_004, Severity::Info)
            .why(format!(
                "XDG_RUNTIME_DIR is unset and {} does not exist; using {} instead, \
                 which is shared and does not survive a reboot",
                run_user.display(),
                path.display()
            ))
            .fix(FixAction::SetEnv {
                key: "XDG_RUNTIME_DIR".to_owned(),
                value: run_user.display().to_string(),
            })
            .build();
        RuntimeDir {
            path,
            diagnostic: Some(diagnostic),
        }
    }

    /// Der gRPC-Socket des Daemons. Rechte `0600` in einem Verzeichnis `0700`.
    #[must_use]
    pub fn daemon_socket(&self) -> PathBuf {
        self.runtime_dir().path.join("daemon.sock")
    }

    /// Das Verzeichnis des Proxy-Sockets.
    ///
    /// Der Proxy-Socket bekommt ein eigenes Verzeichnis, weil er als einziger
    /// Pfad in die Sandbox eingehängt wird. Läge er neben `daemon.sock`, hinge
    /// mit ihm der Daemon-Socket mit drin.
    #[must_use]
    pub fn proxy_socket_dir(&self) -> PathBuf {
        self.runtime_dir().path.join("proxy")
    }

    /// Der Socket, den der Agent in der Sandbox erreicht.
    #[must_use]
    pub fn proxy_socket(&self) -> PathBuf {
        self.proxy_socket_dir().join("proxy.sock")
    }

    /// Das Sitzungs-Token für den Metadaten-Kopf `x-humanitl-token`.
    #[must_use]
    pub fn token_path(&self) -> PathBuf {
        self.runtime_dir().path.join("token")
    }

    fn xdg(&self, variable: &str, fallback: &str) -> PathBuf {
        self.env
            .non_empty(variable)
            .map_or_else(|| self.home().join(fallback), PathBuf::from)
    }

    fn xdg_data(&self) -> PathBuf {
        self.env
            .non_empty("XDG_DATA_HOME")
            .map_or_else(|| self.home().join(".local").join("share"), PathBuf::from)
    }

    fn tmp_base(&self) -> PathBuf {
        self.env
            .non_empty("TMPDIR")
            .map_or_else(|| PathBuf::from("/tmp"), PathBuf::from)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::path::{Path, PathBuf};

    use humanitl_core::Severity;

    use super::Paths;
    use crate::env::Env;

    fn paths(pairs: &[(&str, &str)]) -> Paths {
        Paths::new(Env::from_pairs(pairs.iter().copied()).with_uid(1000))
    }

    #[test]
    fn xdg_variables_win_over_the_home_fallback() {
        let paths = paths(&[
            ("HOME", "/home/x"),
            ("XDG_CONFIG_HOME", "/cfg"),
            ("XDG_DATA_HOME", "/data"),
            ("XDG_RUNTIME_DIR", "/run/user/1000"),
        ]);

        assert_eq!(
            paths.config_path(),
            PathBuf::from("/cfg/humanitl/config.toml")
        );
        assert_eq!(
            paths.rules_path(),
            PathBuf::from("/cfg/humanitl/rules.yaml")
        );
        assert_eq!(
            paths.profile_path("work"),
            PathBuf::from("/cfg/humanitl/profiles/work.toml")
        );
        assert_eq!(paths.db_path(), PathBuf::from("/data/humanitl/humanitl.db"));
        assert_eq!(
            paths.audit_path(),
            PathBuf::from("/data/humanitl/audit/audit.jsonl")
        );
        assert_eq!(
            paths.ca_key_path(),
            PathBuf::from("/data/humanitl/ca/ca.key")
        );
        assert_eq!(
            paths.daemon_socket(),
            PathBuf::from("/run/user/1000/humanitl/daemon.sock")
        );
        assert_eq!(
            paths.proxy_socket(),
            PathBuf::from("/run/user/1000/humanitl/proxy/proxy.sock")
        );
        assert_eq!(
            paths.token_path(),
            PathBuf::from("/run/user/1000/humanitl/token")
        );
    }

    #[test]
    fn without_xdg_the_paths_hang_under_home() {
        let paths = paths(&[("HOME", "/home/x")]);
        assert_eq!(
            paths.config_dir(),
            PathBuf::from("/home/x/.config/humanitl")
        );
        assert_eq!(
            paths.data_dir(),
            PathBuf::from("/home/x/.local/share/humanitl")
        );
        assert_eq!(
            paths.project_profile_path(Path::new("/p")),
            PathBuf::from("/p/.humanitl/profile.toml")
        );
    }

    #[test]
    fn an_empty_variable_counts_as_unset() {
        let paths = paths(&[("HOME", "/home/x"), ("XDG_CONFIG_HOME", "")]);
        assert_eq!(
            paths.config_dir(),
            PathBuf::from("/home/x/.config/humanitl")
        );
    }

    #[test]
    fn runtime_dir_falls_back_to_run_user_then_to_tmp() {
        let paths = paths(&[("HOME", "/home/x")]);

        let run_user = paths.runtime_dir_with(&|path: &Path| path == Path::new("/run/user/1000"));
        assert_eq!(run_user.path, PathBuf::from("/run/user/1000/humanitl"));
        assert!(run_user.diagnostic.is_none());

        let tmp = paths.runtime_dir_with(&|_| false);
        assert_eq!(tmp.path, PathBuf::from("/tmp/humanitl-1000"));
        let Some(diagnostic) = tmp.diagnostic else {
            panic!("the tmp fallback must carry a diagnostic");
        };
        assert_eq!(diagnostic.code.as_str(), "CONFIG_004");
        assert_eq!(diagnostic.title, "Laufzeitverzeichnis ist ein Ersatz");
        assert_eq!(diagnostic.severity, Severity::Info);
        assert!(diagnostic.why.contains("/run/user/1000"));
        assert!(diagnostic.why.contains("/tmp/humanitl-1000"));
    }

    #[test]
    fn tmpdir_moves_the_fallback() {
        let paths = paths(&[("HOME", "/home/x"), ("TMPDIR", "/var/tmp")]);
        let tmp = paths.runtime_dir_with(&|_| false);
        assert_eq!(tmp.path, PathBuf::from("/var/tmp/humanitl-1000"));
    }

    #[test]
    fn a_blob_is_sharded_by_its_first_two_hex_digits() {
        let paths = paths(&[("XDG_DATA_HOME", "/data")]);
        assert_eq!(
            paths.blob_path("ab34cd"),
            PathBuf::from("/data/humanitl/blobs/ab/ab34cd")
        );
        assert_eq!(
            paths.blob_path("a"),
            PathBuf::from("/data/humanitl/blobs/a/a")
        );
    }

    #[test]
    fn without_home_everything_hangs_under_tmp() {
        let paths = paths(&[]);
        assert_eq!(
            paths.config_dir(),
            PathBuf::from("/tmp/humanitl-1000-home/.config/humanitl")
        );
    }
}
