//! Der Probestart eines Sandbox-Backends (HUM-436).
//!
//! `humanitl doctor --probe` startet je zugelassenem, verfügbarem Backend eine
//! leere Sandbox: das mitgelieferte Sandbox-Profil, ein leeres
//! Projektverzeichnis, ein Platzhalter für den Proxy-Socket, eine leere CA,
//! der echte Shim und `true` als Befehl. Danach liest er die fünf
//! `CHECK`-Zeilen des Shims über denselben Weg wie jeder Start
//! ([`SandboxBackend::isolation_check`]) und räumt auf.
//!
//! Der Probestart ist nicht Teil von `SandboxBackends` (HUM-432) und läuft nur
//! auf Wunsch; ohne `--probe` startet der Doctor nichts.

use std::ffi::OsString;
use std::os::unix::fs::{DirBuilderExt as _, PermissionsExt as _};
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use humanitl_config::{Paths, WorkMode};
use humanitl_core::diagnostics::codes::SANDBOX_011;
use humanitl_core::ids::SessionId;
use humanitl_core::{Diagnostic, Severity};

use crate::backend::Backend;
use crate::bridge_env::ExecFailure;
use crate::launcher::{
    CheckResult, IsolationCheck, LaunchOptions, PlanRequest, SandboxBackend, StdioMode,
};
use crate::profile::{SandboxProfile, SessionContext};

/// Der Name des Shims auf dem Host.
const SHIM_BINARY: &str = "humanitl-shim";

/// Wo eine Installation den Shim ablegt, in dieser Reihenfolge.
const SHIM_DIRS: &[&str] = &[
    "/usr/lib/humanitl",
    "/usr/libexec/humanitl",
    "/usr/local/lib/humanitl",
];

/// Wie lange der Probestart nach den `CHECK`-Zeilen auf das Ende von `true`
/// wartet, bevor er die Sandbox beendet.
const EXIT_GRACE: Duration = Duration::from_secs(10);

/// Wie lange der Probestart nach dem Ende auf die letzte Zeile des Berichts
/// wartet: `EXEC fail` kommt nach allen `CHECK`-Zeilen.
const REPORT_TAIL: Duration = Duration::from_secs(2);

/// Wie die Sandbox des Probestarts endete.
#[derive(Debug)]
pub(super) enum Ending {
    /// Beendet: der Status, oder der Befund, den das Ende trug.
    Ended(Result<ExitStatus, Diagnostic>),
    /// Lief nach dieser Frist noch und wurde beendet.
    TimedOut(Duration),
}

/// Was der Probestart gesehen hat.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum ProbeOutcome {
    /// Gestartet, und alle drei Garantien gelten; der Text ist der Beleg.
    Passed(String),
    /// Der Plan oder der Start scheiterte; der Befund des Backends.
    NotStarted(Diagnostic),
    /// Gestartet, aber `true` lief nicht durch: kein Bericht über alle
    /// Garantien, `EXEC fail`, kein Ende in der Frist oder ein Status außer 0.
    Incomplete {
        /// Was fehlte, als Satzteil nach „started, and“.
        what: String,
        /// Der Befund dazu, wenn es einen gab.
        inner: Option<Diagnostic>,
    },
    /// Gestartet, aber eine Garantie gilt nicht: ihr Name, ihr Beleg und ihr
    /// Befund.
    Failed {
        /// Die Garantie, etwa `single_socket`.
        check: &'static str,
        /// Was der Shim dazu schrieb.
        evidence: String,
        /// Der Befund der Garantie, wenn es einen gab.
        diagnostic: Option<Diagnostic>,
    },
}

/// Startet eine leere Sandbox mit `backend` und dem Shim `shim` und prüft die
/// drei Garantien aus den fünf `CHECK`-Zeilen.
///
/// Blockiert, bis `true` beendet ist oder die Frist des Berichts abläuft.
/// Projekt und CA liegen in einem eigenen Verzeichnis unter dem temporären
/// Verzeichnis, der Platzhalter des Proxy-Sockets unter eigenem Namen im
/// Proxy-Verzeichnis: Nur von dort hängt ein Backend einen Socket ein
/// (`SANDBOX_006`). Alles verschwindet danach wieder; der Socket des Proxys
/// einer laufenden Sitzung bleibt unberührt.
pub(super) fn probe_start(backend: &Backend, shim: &Path) -> ProbeOutcome {
    let scratch = match Scratch::create(backend.paths()) {
        Ok(scratch) => scratch,
        Err(diagnostic) => return ProbeOutcome::NotStarted(diagnostic),
    };
    match start(backend, shim, &scratch) {
        Ok(outcome) | Err(outcome) => outcome,
    }
}

/// Plan, Start, Bericht und Ende; jeder frühe Ausgang ist ein Ergebnis.
fn start(backend: &Backend, shim: &Path, scratch: &Scratch) -> Result<ProbeOutcome, ProbeOutcome> {
    let profile = SandboxProfile::parse(
        include_str!("../../../../../profiles/sandbox/default.toml"),
        Path::new("default.toml"),
    )
    .map_err(ProbeOutcome::NotStarted)?;
    // Der Platzhalter lebt bis zum Ende dieser Funktion. Hinter ihm antwortet
    // niemand; der Shim braucht nur die gebundene Datei, um seine Brücke
    // aufzumachen, wie im Escape-Harness.
    let _listener = UnixListener::bind(&scratch.socket)
        .map_err(|err| ProbeOutcome::NotStarted(failure(&scratch.socket, &err.to_string())))?;
    std::fs::set_permissions(&scratch.socket, std::fs::Permissions::from_mode(0o600))
        .map_err(|err| ProbeOutcome::NotStarted(failure(&scratch.socket, &err.to_string())))?;
    let context = SessionContext {
        session: SessionId::new(),
        work_src: scratch.work.clone(),
        work_mode: WorkMode::Rw,
        proxy_socket_src: scratch.socket.clone(),
        ca_cert_src: scratch.ca.clone(),
        ca_bundle_src: scratch.ca.clone(),
        shim_src: shim.to_path_buf(),
        session_env: vec![("HUMANITL_SESSION".to_owned(), "doctor-probe".to_owned())],
        command: vec![OsString::from("true")],
        files: Vec::new(),
        agent_program: None,
    };
    let options = LaunchOptions {
        stdio: StdioMode::Capture,
        ..LaunchOptions::default()
    };
    let plan = backend
        .plan(&PlanRequest::new(&profile, &context, &options))
        .map_err(ProbeOutcome::NotStarted)?;
    let handle = backend.launch(&plan).map_err(ProbeOutcome::NotStarted)?;
    let results = backend.isolation_check(&handle);
    let ending = handle.wait_timeout(EXIT_GRACE).map_or_else(
        || {
            let _ = handle.kill();
            let _ = handle.wait();
            Ending::TimedOut(EXIT_GRACE)
        },
        Ending::Ended,
    );
    let report = handle.report_after_exit(REPORT_TAIL);
    let label = format!("{} {}", backend.name(), backend.program().display());
    Ok(verdict(
        &results,
        &ending,
        report.exec_failed.as_ref(),
        &label,
    ))
}

/// Das Urteil über einen Probestart, der lief.
///
/// Bestanden ist er nur, wenn alles zusammenkommt: ein Ergebnis je Garantie
/// aus den fünf `CHECK`-Zeilen und jede davon gültig, kein `EXEC fail`, ein
/// Ende innerhalb der Frist und Status 0. Sonst landet nichts im
/// Zwischenspeicher, und die Zeile trägt `DOCTOR_014` mit dem, was fehlte.
pub(super) fn verdict(
    results: &[CheckResult],
    ending: &Ending,
    exec_failed: Option<&ExecFailure>,
    label: &str,
) -> ProbeOutcome {
    if results.len() != IsolationCheck::ALL.len() {
        return ProbeOutcome::Incomplete {
            what: format!(
                "the shim reported {} of {} guarantees",
                results.len(),
                IsolationCheck::ALL.len()
            ),
            inner: None,
        };
    }
    if let Some(failed) = results.iter().find(|result| !result.passed) {
        return ProbeOutcome::Failed {
            check: failed.check.as_str(),
            evidence: failed.evidence.clone(),
            diagnostic: failed.diagnostic.clone(),
        };
    }
    if let Some(failure) = exec_failed {
        return ProbeOutcome::Incomplete {
            what: failure.errno.map_or_else(
                || "the shim could not run true (EXEC fail)".to_owned(),
                |errno| format!("the shim could not run true (EXEC fail, errno {errno})"),
            ),
            inner: None,
        };
    }
    match ending {
        Ending::TimedOut(after) => ProbeOutcome::Incomplete {
            what: format!("true did not end within {} ms", after.as_millis()),
            inner: None,
        },
        Ending::Ended(Err(diagnostic)) => ProbeOutcome::Incomplete {
            what: "its end carried a finding".to_owned(),
            inner: Some(diagnostic.clone()),
        },
        Ending::Ended(Ok(status)) if !status.success() => ProbeOutcome::Incomplete {
            what: format!("true ended with {status}"),
            inner: None,
        },
        Ending::Ended(Ok(_)) => {
            let passed: Vec<&str> = results.iter().map(|result| result.check.as_str()).collect();
            ProbeOutcome::Passed(format!(
                "true ran under {label}; {} hold",
                passed.join(", ")
            ))
        }
    }
}

/// Was der Probestart anlegt: Projekt und CA in einem Verzeichnis, nur für
/// diesen Prozess lesbar, und den Platzhalter des Proxy-Sockets. Alles ist
/// weg, sobald der Wert fällt.
struct Scratch {
    dir: PathBuf,
    work: PathBuf,
    socket: PathBuf,
    ca: PathBuf,
}

impl Scratch {
    /// Ein neues, leeres Verzeichnis mit Rechten `0700` und ein freier Name
    /// im Proxy-Verzeichnis dieser Pfade. Kurz benannt, weil der Pfad eines
    /// Sockets unter macOS höchstens 104 Bytes lang sein darf.
    fn create(paths: &Paths) -> Result<Self, Diagnostic> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos());
        let tag = format!("{}-{nanos}", std::process::id());
        let dir = std::env::temp_dir().join(format!("hl-probe-{tag}"));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&dir)
            .map_err(|err| failure(&dir, &err.to_string()))?;
        // Der Wert entsteht sofort nach dem Verzeichnis: Scheitert ein
        // späterer Schritt, räumt sein `Drop` das Verzeichnis wieder weg.
        let proxy_dir = paths.proxy_socket_dir();
        let scratch = Self {
            work: dir.join("work"),
            socket: proxy_dir.join(format!("probe-{tag}.sock")),
            ca: dir.join("ca.crt"),
            dir,
        };
        std::fs::DirBuilder::new()
            .mode(0o700)
            .recursive(true)
            .create(&proxy_dir)
            .map_err(|err| failure(&proxy_dir, &err.to_string()))?;
        std::fs::create_dir(&scratch.work)
            .map_err(|err| failure(&scratch.work, &err.to_string()))?;
        std::fs::write(&scratch.ca, b"").map_err(|err| failure(&scratch.ca, &err.to_string()))?;
        Ok(scratch)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `SANDBOX_011`: Der Probestart konnte seinen Platz nicht anlegen.
fn failure(path: &Path, error: &str) -> Diagnostic {
    Diagnostic::builder(SANDBOX_011, Severity::Blocking)
        .why(format!(
            "the probe start could not prepare {}: {error}",
            path.display()
        ))
        .fix(super::command_fix(&[
            "ls",
            "-ld",
            &path.parent().unwrap_or(path).to_string_lossy(),
        ]))
        .build()
}

/// Der Shim auf dem Host: neben dem laufenden Programm, ein Verzeichnis
/// darüber (ein Testbinary liegt in `target/<profil>/deps/`), sonst in einer
/// Installation. Gibt es keinen, der erste Kandidat; der Start sagt dann mit
/// seinem Befund, dass er fehlt.
#[must_use]
pub fn locate_shim() -> PathBuf {
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        candidates.push(dir.join(SHIM_BINARY));
        if let Some(up) = dir.parent() {
            candidates.push(up.join(SHIM_BINARY));
        }
    }
    candidates.extend(SHIM_DIRS.iter().map(|dir| Path::new(dir).join(SHIM_BINARY)));
    candidates
        .iter()
        .find(|path| crate::agent::is_executable_file(path))
        .cloned()
        .or_else(|| candidates.first().cloned())
        .unwrap_or_else(|| Path::new(SHIM_DIRS[0]).join(SHIM_BINARY))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::process::ExitStatusExt as _;
    use std::process::ExitStatus;
    use std::time::Duration;

    use super::{Ending, ProbeOutcome, verdict};
    use crate::bridge_env::ExecFailure;
    use crate::launcher::{CheckResult, IsolationCheck};

    fn all_passed() -> Vec<CheckResult> {
        IsolationCheck::ALL
            .iter()
            .map(|&check| CheckResult {
                check,
                passed: true,
                evidence: "ok".to_owned(),
                diagnostic: None,
            })
            .collect()
    }

    fn exited(code: i32) -> Ending {
        Ending::Ended(Ok(ExitStatus::from_raw(code << 8)))
    }

    fn incomplete(outcome: &ProbeOutcome) -> &str {
        match outcome {
            ProbeOutcome::Incomplete { what, .. } => what,
            other => panic!("not incomplete: {other:?}"),
        }
    }

    #[test]
    fn three_guarantees_an_exec_and_exit_0_pass() {
        let outcome = verdict(&all_passed(), &exited(0), None, "bwrap /usr/bin/bwrap");
        assert!(matches!(outcome, ProbeOutcome::Passed(_)), "{outcome:?}");
    }

    /// Mutationsbeweis: Übergeht das Urteil den Status, besteht ein
    /// Probestart, dessen `true` mit 1 endete, und landet im
    /// Zwischenspeicher.
    #[test]
    fn a_nonzero_exit_is_no_pass() {
        let outcome = verdict(&all_passed(), &exited(1), None, "x");
        assert!(incomplete(&outcome).contains("ended with"), "{outcome:?}");
    }

    #[test]
    fn a_timeout_an_exec_failure_and_a_short_report_are_no_pass() {
        let timed_out = verdict(
            &all_passed(),
            &Ending::TimedOut(Duration::from_secs(10)),
            None,
            "x",
        );
        assert!(incomplete(&timed_out).contains("10000 ms"), "{timed_out:?}");

        let failure = ExecFailure { errno: Some(2) };
        let missing = verdict(&all_passed(), &exited(0), Some(&failure), "x");
        assert!(incomplete(&missing).contains("errno 2"), "{missing:?}");

        let short = verdict(&all_passed()[..2], &exited(0), None, "x");
        assert!(incomplete(&short).contains("2 of 3"), "{short:?}");
        let none = verdict(&[], &exited(0), None, "x");
        assert!(incomplete(&none).contains("0 of 3"), "{none:?}");
    }

    #[test]
    fn a_broken_guarantee_names_itself() {
        let mut results = all_passed();
        results[1].passed = false;
        let outcome = verdict(&results, &exited(0), None, "x");
        assert!(
            matches!(outcome, ProbeOutcome::Failed { check, .. } if check == results[1].check.as_str()),
            "{outcome:?}"
        );
    }
}
