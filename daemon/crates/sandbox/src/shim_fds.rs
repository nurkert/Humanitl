//! Lebensleine und Umgebung für den Shim über geerbte Deskriptoren (HUM-430).
//!
//! Zwei Dinge, die bwrap mit Flags erledigt, braucht ein Backend, das die
//! Sandbox vom Daemon abgekoppelt startet und jeden Wert seiner Zeile auf die
//! Platte schreibt (eine Container-Engine, HUM-292 P7 und P11), als Deskriptor:
//!
//! - [`Lifeline`]: die Lebensleine statt `--die-with-parent`. Eine Pipe, deren
//!   Leseende der Shim mit [`crate::SHIM_LIFELINE_FD`] erbt und deren
//!   Schreibende allein der Daemon hält, mit `CLOEXEC`. Endet der Daemon, auch
//!   durch `SIGKILL`, schließt der Kernel das Schreibende, das Leseende meldet
//!   `POLLHUP`, und der Shim beendet die Sandbox wie bei `SIGTERM`. Das
//!   Leseende darf in weiteren Prozessen offen sein (im Client der Engine);
//!   entscheidend ist allein, dass das Schreibende nur im Daemon liegt.
//! - [`env_memfd`]: die Umgebung des Agenten als JSON-Objekt in einem
//!   versiegelten memfd, den der Shim mit [`crate::SHIM_ENV_FD`] erbt, statt
//!   als Werte in `argv` ([`crate::launcher`], `EnvDelivery::InheritedFd`).
//!   Die fünf Variablen des Shims ([`crate::RESERVED_ENV`]) gehören nicht
//!   hinein: Sie sind Vertrag zwischen Launcher und Shim, keine Umgebung des
//!   Agenten, und der Shim lehnt sie im memfd ab ([`split_env`] trennt sie).
//!
//! Beide Deskriptoren erreicht der Agent nie: Der Shim schließt sie vor dem
//! `exec`, wie den Berichts-Deskriptor (`docs/SECURITY.md` Abschnitt 4).

use std::fmt;
use std::io::{self, PipeWriter};
use std::os::fd::OwnedFd;

use humanitl_core::diagnostics::codes::{SANDBOX_011, SANDBOX_012};
use humanitl_core::{Diagnostic, Severity};

use crate::bridge_env::RESERVED_ENV;

/// Der Name des memfd mit der Umgebung, wie ihn `/proc/<pid>/fd` zeigt.
pub const ENV_MEMFD_NAME: &str = "humanitl-env";

/// Eine Umgebung als Paare `(Schlüssel, Wert)`.
pub type EnvPairs = Vec<(String, String)>;

/// Trennt eine Umgebung in den Teil des Agenten und die Variablen des Shims
/// ([`RESERVED_ENV`]), als `(agent, shim)`, jeweils in der Reihenfolge der
/// Eingabe.
///
/// Der Teil des Agenten geht in [`env_memfd`], die Variablen des Shims in
/// dessen eigene Umgebung: Sie sind keine Geheimnisse, und der Shim braucht
/// den Berichts-Deskriptor, bevor er den memfd liest, damit ein kaputter
/// memfd eine Prüfzeile wird.
#[must_use]
pub fn split_env(env: &[(String, String)]) -> (EnvPairs, EnvPairs) {
    env.iter()
        .cloned()
        .partition(|(key, _)| !RESERVED_ENV.contains(&key.as_str()))
}

/// Die Umgebung des Agenten als versiegelter memfd: ein JSON-Objekt,
/// `String` zu `String`, Leseposition am Anfang, `CLOEXEC`, nie auf 0, 1
/// oder 2.
///
/// # Errors
///
/// - `SANDBOX_012`, wenn ein Schlüssel aus [`RESERVED_ENV`] oder einer der
///   Lader-Variablen (`humanitl_config::LOADER_ENV_KEYS`) darin steht, oder
///   ein Schlüssel zweimal: ein Fehler des Starters, den der Shim ebenso
///   ablehnte ([`split_env`] trennt die Variablen des Shims ab, das Profil
///   lehnt die Lader-Variablen beim Laden ab).
/// - `SANDBOX_011`, wenn der memfd sich nicht anlegen, füllen oder
///   versiegeln lässt.
pub fn env_memfd(env: &[(String, String)]) -> Result<OwnedFd, Diagnostic> {
    let mut object = serde_json::Map::new();
    for (key, value) in env {
        let refused = if RESERVED_ENV.contains(&key.as_str()) {
            Some(
                "belongs to the launcher contract and reaches the shim through its own environment",
            )
        } else if humanitl_config::is_loader_key(key) {
            Some("would load code before the agent's filter")
        } else if object.contains_key(key) {
            Some("is given twice")
        } else {
            None
        };
        if let Some(why) = refused {
            return Err(Diagnostic::builder(SANDBOX_012, Severity::Blocking)
                .why(format!(
                    "the agent's environment cannot go to the shim: {key} {why}"
                ))
                .build());
        }
        object.insert(key.clone(), serde_json::Value::String(value.clone()));
    }
    let text = serde_json::Value::Object(object).to_string();
    crate::platform::sealed_memfd(ENV_MEMFD_NAME, text.as_bytes()).map_err(|why| {
        Diagnostic::builder(SANDBOX_011, Severity::Blocking)
            .why(why)
            .build()
    })
}

/// Das Schreibende der Lebensleine, im Daemon.
///
/// Solange der Wert lebt, lebt die Sandbox, die das Leseende erbte; fällt er
/// (oder endet der Daemon, auch durch `SIGKILL`), beendet der Shim sie. Das
/// Schreibende trägt `CLOEXEC` und wird nie vererbt: kein Kind des Daemons,
/// auch nicht das gestartete Programm, hält die Sandbox am Leben. Beschrieben
/// wird es nie.
pub struct Lifeline {
    _write: PipeWriter,
}

impl Lifeline {
    /// Eine neue Lebensleine und ihr Leseende für den Shim.
    ///
    /// Das Leseende trägt `CLOEXEC` und liegt nie auf 0, 1 oder 2; es geht
    /// unter die geerbten Deskriptoren des Plans und mit
    /// [`crate::SHIM_LIFELINE_FD`] an den Shim.
    ///
    /// # Errors
    ///
    /// Der Fehler von `pipe2(2)` oder vom Umlegen über die Standardströme.
    pub fn new() -> io::Result<(Self, OwnedFd)> {
        let (read, write) = io::pipe()?;
        let read = crate::inherit::above_stdio(OwnedFd::from(read))?;
        Ok((Self { _write: write }, read))
    }
}

impl fmt::Debug for Lifeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Lifeline")
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::io::Read as _;
    use std::os::fd::AsFd as _;

    use rustix::io::{FdFlags, fcntl_getfd};

    use super::*;
    use crate::bridge_env::ENV_REPORT_FD;

    fn pairs(list: &[(&str, &str)]) -> Vec<(String, String)> {
        list.iter()
            .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
            .collect()
    }

    #[test]
    fn the_shims_variables_are_split_from_the_agents() {
        let (agent, shim) = split_env(&pairs(&[
            ("HOME", "/home/agent"),
            (ENV_REPORT_FD, "5"),
            ("PATH", "/usr/bin"),
        ]));
        assert_eq!(
            agent,
            pairs(&[("HOME", "/home/agent"), ("PATH", "/usr/bin")])
        );
        assert_eq!(shim, pairs(&[(ENV_REPORT_FD, "5")]));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_environment_memfd_is_one_sealed_json_object() {
        let fd = env_memfd(&pairs(&[
            ("HL_SECRET", "a b \"c\"\nd"),
            ("PATH", "/usr/bin"),
        ]))
        .unwrap();
        assert!(fcntl_getfd(fd.as_fd()).unwrap().contains(FdFlags::CLOEXEC));
        let mut file = std::fs::File::from(fd);
        let mut text = String::new();
        file.read_to_string(&mut text).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"HL_SECRET": "a b \"c\"\nd", "PATH": "/usr/bin"})
        );
        let err = std::io::Write::write_all(&mut file, b"{}").unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::PermissionDenied);
    }

    /// `serde_json` writes DEL and the C1 controls raw, not escaped; the
    /// shim's reader must therefore take them (`json.rs` in the shim, and
    /// the contract test `the_shim_reads_the_launchers_environment_memfd`).
    #[cfg(target_os = "linux")]
    #[test]
    fn del_and_c1_controls_stand_raw_in_the_memfd() {
        let fd = env_memfd(&pairs(&[("A", "x\u{7f}\u{85}y")])).unwrap();
        let mut text = String::new();
        std::fs::File::from(fd).read_to_string(&mut text).unwrap();
        assert_eq!(text, "{\"A\":\"x\u{7f}\u{85}y\"}");
    }

    #[test]
    fn reserved_loader_and_twice_given_keys_are_refused() {
        for (list, needle) in [
            (vec![(ENV_REPORT_FD, "5")], "launcher contract"),
            (vec![("LD_PRELOAD", "/tmp/x.so")], "load code"),
            (vec![("A", "1"), ("A", "2")], "twice"),
        ] {
            let diagnostic = env_memfd(&pairs(&list)).unwrap_err();
            assert_eq!(diagnostic.code, SANDBOX_012);
            assert!(diagnostic.why.contains(needle), "{}", diagnostic.why);
        }
    }

    #[test]
    fn the_lifeline_read_end_sees_its_end_when_the_lifeline_is_dropped() {
        let (lifeline, read) = Lifeline::new().unwrap();
        assert!(
            fcntl_getfd(read.as_fd())
                .unwrap()
                .contains(FdFlags::CLOEXEC)
        );
        assert!(std::os::fd::AsRawFd::as_raw_fd(&read) > 2);
        drop(lifeline);
        let mut rest = Vec::new();
        std::fs::File::from(read).read_to_end(&mut rest).unwrap();
        assert!(rest.is_empty());
    }
}
