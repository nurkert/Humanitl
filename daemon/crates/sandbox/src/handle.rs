//! Die laufende Sandbox: warten, beenden, den Bericht lesen.
//!
//! Ein [`SandboxHandle`] gehört zu einem `bwrap`-Prozess auf dem Host. Der
//! Prozess selbst liegt bei dem Thread, der ihn gestartet hat und auf ihn
//! wartet ([`SandboxBackend::launch`](crate::SandboxBackend::launch)); das Handle sieht nur den
//! geteilten Zustand: den Exit-Status, sobald es einen gibt, die Zeilen des
//! Shim-Berichts, die `bwrap`-Statusmeldungen und, wenn gesammelt, die
//! Ausgabe. Deshalb sind alle Methoden `&self`, und das Handle lässt sich
//! zwischen Threads teilen: `wait` in einem, `kill` in einem anderen.
//!
//! # Lief der Befehl, oder ist `bwrap` vorher gescheitert?
//!
//! `bwrap` meldet über `--json-status-fd` die PID seines Kindes, sobald es
//! den Namensraum betreten hat, also *vor* den Mounts; ein `--ro-bind` auf
//! eine Quelle, die es nicht gibt, scheitert danach. Erst wenn der Befehl
//! ausgeführt wurde, meldet `bwrap` am Ende `{"exit-code": N}`. Daraus folgt
//! der Befund von [`SandboxHandle::wait`]: endet `bwrap` mit einem Exit-Code,
//! aber ohne diese Zeile, hat der Befehl nie gestartet, und das ist ein
//! `SANDBOX_012` (oder `SANDBOX_003`, wenn die Fehlerausgabe von
//! Nutzer-Namensräumen spricht), nicht ein Befehl, der mit 1 endete. Ein
//! `bwrap`, das ein Signal beendet hat (etwa [`SandboxHandle::kill`]), ist kein
//! Startfehler.

use std::process::ExitStatus;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes::SANDBOX_012;
use humanitl_core::ids::SandboxId;
use humanitl_core::{Diagnostic, Severity};
use rustix::process::{Pid, Signal, kill_process, kill_process_group};

use crate::bridge_env::{CHECK_NAMES, ShimCheck};
use crate::bwrap::{is_userns_failure, userns_diagnostic};

/// Wie lange [`SandboxHandle::kill`] nach `SIGTERM` wartet, bevor `SIGKILL` folgt.
pub const KILL_GRACE: Duration = Duration::from_secs(5);

/// Wie lange [`SandboxHandle::interrupt`] nach `SIGINT` auf das Ende der
/// Sandbox wartet, bevor der Aufrufer eskaliert.
pub const INTERRUPT_GRACE: Duration = Duration::from_secs(5);

/// Wie lange nach dem Ende von `bwrap` auf die letzte Zeile der Status-Pipe
/// gewartet wird, bevor der Befund gefällt wird. Die Pipe schließt mit
/// `bwrap`; die Frist deckt nur den Leser-Thread, der der Zeile hinterher
/// sein könnte.
pub const STATUS_DRAIN: Duration = Duration::from_secs(1);

/// Wie viel gesammelte Ausgabe je Strom behalten wird, wenn
/// [`crate::StdioMode::Capture`] gilt. Was darüber hinausgeht, wird gelesen
/// und verworfen, damit die Sandbox nicht an einer vollen Pipe hängt.
pub const CAPTURE_MAX_BYTES: usize = 1 << 20;

/// Wie viel Fehlerausgabe ein Befund höchstens zitiert.
pub const STDERR_EXCERPT_BYTES: usize = 2048;

/// Die gesammelte Ausgabe einer Sandbox mit [`crate::StdioMode::Capture`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CapturedOutput {
    /// Alles, was auf stdout kam, bis [`CAPTURE_MAX_BYTES`].
    pub stdout: Vec<u8>,
    /// Alles, was auf stderr kam, bis [`CAPTURE_MAX_BYTES`].
    pub stderr: Vec<u8>,
}

/// Was der Shim bis jetzt gemeldet hat.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ReportSnapshot {
    /// Die gelesenen Zeilen, in Reihenfolge.
    pub checks: Vec<ShimCheck>,
    /// Zeilen, die keine `CHECK`-Zeile waren.
    pub other_lines: usize,
    /// Die Pipe ist zu: alle Schreibseiten sind geschlossen.
    pub closed: bool,
}

impl ReportSnapshot {
    /// Ob jeder Name aus [`CHECK_NAMES`] gemeldet wurde.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        CHECK_NAMES
            .iter()
            .all(|name| self.checks.iter().any(|check| check.name == *name))
    }

    /// Die Zeile zu einem Namen, die letzte, wenn es mehrere gibt.
    #[must_use]
    pub fn get(&self, name: &str) -> Option<&ShimCheck> {
        self.checks.iter().rev().find(|check| check.name == name)
    }
}

/// Was `bwrap` über `--json-status-fd` gemeldet hat.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StatusSnapshot {
    /// Die PID des Kindes von `bwrap` auf dem Host (das Init des
    /// PID-Namensraums); gesetzt, sobald `bwrap` den Namensraum betreten hat,
    /// noch vor den Mounts.
    pub child_pid: Option<u32>,
    /// Der Exit-Code, den `bwrap` gemeldet hat. Nur vorhanden, wenn der
    /// Befehl ausgeführt wurde.
    pub exit_code: Option<i32>,
    /// Die Pipe ist zu.
    pub closed: bool,
}

/// Der geteilte Zustand hinter dem Handle.
#[derive(Debug, Default)]
pub(crate) struct Shared {
    exit: Mutex<Option<ExitStatus>>,
    exited: Condvar,
    report: Mutex<ReportSnapshot>,
    report_changed: Condvar,
    status: Mutex<StatusSnapshot>,
    status_changed: Condvar,
    stdout: Mutex<Vec<u8>>,
    stderr: Mutex<Vec<u8>>,
    /// Die Leser der gesammelten Ausgabe; werden vor [`SandboxHandle::output`]
    /// eingesammelt, damit die Ausgabe vollständig ist.
    readers: Mutex<Vec<JoinHandle<()>>>,
    capturing: Mutex<bool>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    pub(crate) fn set_exit(&self, status: ExitStatus) {
        *lock(&self.exit) = Some(status);
        self.exited.notify_all();
    }

    pub(crate) fn push_check(&self, check: ShimCheck) {
        lock(&self.report).checks.push(check);
        self.report_changed.notify_all();
    }

    pub(crate) fn push_other_line(&self) {
        lock(&self.report).other_lines += 1;
        self.report_changed.notify_all();
    }

    pub(crate) fn close_report(&self) {
        lock(&self.report).closed = true;
        self.report_changed.notify_all();
    }

    pub(crate) fn set_child_pid(&self, pid: u32) {
        lock(&self.status).child_pid = Some(pid);
        self.status_changed.notify_all();
    }

    pub(crate) fn set_exit_code(&self, code: i32) {
        lock(&self.status).exit_code = Some(code);
        self.status_changed.notify_all();
    }

    pub(crate) fn close_status(&self) {
        lock(&self.status).closed = true;
        self.status_changed.notify_all();
    }

    pub(crate) fn append_stdout(&self, chunk: &[u8]) {
        append_capped(&mut lock(&self.stdout), chunk);
    }

    pub(crate) fn append_stderr(&self, chunk: &[u8]) {
        append_capped(&mut lock(&self.stderr), chunk);
    }

    pub(crate) fn add_reader(&self, reader: JoinHandle<()>) {
        lock(&self.readers).push(reader);
    }

    pub(crate) fn set_capturing(&self) {
        *lock(&self.capturing) = true;
    }

    /// Wartet, bis der Exit-Status da ist, höchstens `timeout`.
    pub(crate) fn wait_exit(&self, timeout: Option<Duration>) -> Option<ExitStatus> {
        let deadline = timeout.map(|t| Instant::now() + t);
        let mut exit = lock(&self.exit);
        loop {
            if let Some(status) = *exit {
                return Some(status);
            }
            match deadline {
                None => {
                    exit = self
                        .exited
                        .wait(exit)
                        .unwrap_or_else(PoisonError::into_inner);
                }
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        return None;
                    }
                    exit = self
                        .exited
                        .wait_timeout(exit, deadline - now)
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
            }
        }
    }

    /// Wartet, bis `done` auf dem Bericht wahr ist, die Pipe zu ist, die
    /// Sandbox beendet ist oder `timeout` um ist; liefert den Stand.
    pub(crate) fn wait_report(
        &self,
        timeout: Duration,
        done: impl Fn(&ReportSnapshot) -> bool,
    ) -> ReportSnapshot {
        let deadline = Instant::now() + timeout;
        let mut report = lock(&self.report);
        loop {
            if done(&report) || report.closed || lock(&self.exit).is_some() {
                return report.clone();
            }
            let now = Instant::now();
            if now >= deadline {
                return report.clone();
            }
            // Ein kurzes Intervall, damit auch das Ende der Sandbox das Warten
            // beendet, obwohl es über eine andere Bedingungsvariable kommt.
            let slice = (deadline - now).min(Duration::from_millis(50));
            report = self
                .report_changed
                .wait_timeout(report, slice)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    /// Wartet, bis die Status-Pipe zu ist oder der Exit-Code gemeldet wurde,
    /// höchstens `timeout`. Das Ende der Sandbox beendet das Warten nicht:
    /// genau dann liest der Status-Leser noch die letzte Zeile.
    pub(crate) fn wait_status_settled(&self, timeout: Duration) -> StatusSnapshot {
        let deadline = Instant::now() + timeout;
        let mut status = lock(&self.status);
        loop {
            if status.closed || status.exit_code.is_some() {
                return status.clone();
            }
            let now = Instant::now();
            if now >= deadline {
                return status.clone();
            }
            status = self
                .status_changed
                .wait_timeout(status, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    pub(crate) fn join_readers(&self) {
        let readers: Vec<JoinHandle<()>> = std::mem::take(&mut *lock(&self.readers));
        for reader in readers {
            let _ = reader.join();
        }
    }

    pub(crate) fn stderr_excerpt(&self) -> String {
        let stderr = lock(&self.stderr);
        let cut = stderr.len().min(STDERR_EXCERPT_BYTES);
        String::from_utf8_lossy(&stderr[..cut]).trim().to_owned()
    }

    /// Der Befund zu einem beendeten `bwrap`, siehe Modulbeschreibung.
    ///
    /// Wartet zuvor bis [`STATUS_DRAIN`] auf die letzte Zeile der Status-Pipe
    /// und sammelt die Leser der Ausgabe ein, damit die Fehlerausgabe im
    /// Befund vollständig ist.
    pub(crate) fn verdict(&self, status: ExitStatus) -> Result<ExitStatus, Diagnostic> {
        let Some(code) = status.code() else {
            // Ein Signal: `kill`, oder der Daemon hat aufgeräumt. Kein
            // Startfehler.
            return Ok(status);
        };
        let reported = self.wait_status_settled(STATUS_DRAIN);
        if reported.exit_code.is_some() {
            return Ok(status);
        }
        self.join_readers();
        let stderr = self.stderr_excerpt();
        if is_userns_failure(&stderr) {
            return Err(userns_diagnostic(&stderr));
        }
        let mut why =
            format!("bwrap exited with code {code} before starting the command in the sandbox");
        if stderr.is_empty() {
            if !*lock(&self.capturing) {
                why.push_str("; its message went to the inherited stderr");
            }
        } else {
            why.push_str(": ");
            why.push_str(&stderr);
        }
        Err(Diagnostic::builder(SANDBOX_012, Severity::Blocking)
            .why(why)
            .build())
    }
}

fn append_capped(buffer: &mut Vec<u8>, chunk: &[u8]) {
    let room = CAPTURE_MAX_BYTES.saturating_sub(buffer.len());
    buffer.extend_from_slice(&chunk[..chunk.len().min(room)]);
}

/// Eine gestartete Sandbox.
#[derive(Debug, Clone)]
pub struct SandboxHandle {
    /// Die Id dieser Sandbox.
    pub id: SandboxId,
    /// Die PID des `bwrap`-Prozesses auf dem Host.
    pub pid: u32,
    /// Die Kommandozeile, nach POSIX zitiert, mit Programm; für die
    /// Oberfläche und `humanitl sandbox argv`.
    pub argv_display: String,
    shared: Arc<Shared>,
}

impl SandboxHandle {
    pub(crate) fn new(id: SandboxId, pid: u32, argv_display: String, shared: Arc<Shared>) -> Self {
        Self {
            id,
            pid,
            argv_display,
            shared,
        }
    }

    /// Wartet auf das Ende der Sandbox und liefert den Status von `bwrap`,
    /// der der des Befehls in der Sandbox ist (Signal: 128 + Nummer).
    ///
    /// # Errors
    ///
    /// `SANDBOX_012`, wenn `bwrap` endete, ohne den Befehl je gestartet zu
    /// haben (mit seiner Fehlerausgabe, wenn sie gesammelt wurde), oder
    /// `SANDBOX_003`, wenn diese Fehlerausgabe von Nutzer-Namensräumen
    /// spricht. Siehe Modulbeschreibung.
    pub fn wait(&self) -> Result<ExitStatus, Diagnostic> {
        // `wait_exit(None)` kehrt erst mit einem Status zurück.
        let status = self.shared.wait_exit(None).unwrap_or_default();
        let verdict = self.shared.verdict(status);
        self.shared.join_readers();
        verdict
    }

    /// Wie [`SandboxHandle::wait`], aber höchstens `timeout` lang; `None`,
    /// wenn die Sandbox danach noch läuft.
    #[must_use]
    pub fn wait_timeout(&self, timeout: Duration) -> Option<Result<ExitStatus, Diagnostic>> {
        let status = self.shared.wait_exit(Some(timeout))?;
        let verdict = self.shared.verdict(status);
        self.shared.join_readers();
        Some(verdict)
    }

    /// Der Status, wenn die Sandbox schon beendet ist, ohne Befund.
    #[must_use]
    pub fn try_wait(&self) -> Option<ExitStatus> {
        self.shared.wait_exit(Some(Duration::ZERO))
    }

    /// Beendet die Sandbox: `SIGTERM`, nach [`KILL_GRACE`] `SIGKILL`.
    ///
    /// Kehrt zurück, wenn der Prozess weg ist. `bwrap` reicht `SIGTERM`
    /// nicht an das Kind durch, aber mit `--die-with-parent` endet mit `bwrap`
    /// der ganze PID-Namensraum; `SIGKILL` an `bwrap` beendet deshalb auch den
    /// Agenten.
    pub fn kill(&self) {
        self.terminate(KILL_GRACE);
    }

    /// Bittet den Agenten mit `SIGINT`, selbst aufzuhören, und wartet
    /// höchstens `grace` auf das Ende der Sandbox.
    ///
    /// Gibt `true` zurück, wenn die Sandbox in der Frist beendet ist; sonst
    /// `false`, und der Aufrufer eskaliert mit [`SandboxHandle::kill`].
    ///
    /// Das Signal geht an die Prozessgruppe des Sandbox-Init, nicht an
    /// `bwrap`. Das hat zwei Gründe, und beide sind gemessen:
    ///
    /// - `bwrap` selbst hat für `SIGINT` keinen Handler. Ein `SIGINT` an den
    ///   `bwrap`-Prozess beendet ihn sofort, und mit `--die-with-parent`
    ///   bekommt der Namensraum darunter ein `SIGKILL`. Der Agent käme nie
    ///   dazu, aufzuräumen.
    /// - Das Init des PID-Namensraums ist wieder `bwrap`
    ///   ([`SandboxHandle::child_pid`]), und ein Signal mit Standardwirkung an
    ///   ein Namensraum-Init verwirft der Kernel. Es trägt aber wegen
    ///   `--new-session` die Sitzung und die Prozessgruppe der Sandbox, und
    ///   der Agent hängt darin. `kill(-child_pid, SIGINT)` erreicht deshalb
    ///   genau den Agenten, dessen `SIGINT` der Shim an sein Kind weiterreicht.
    ///
    /// Die eigene Prozessgruppe ist nie betroffen: das Sandbox-Init hat mit
    /// `setsid` eine eigene aufgemacht. Kennt das Handle die PID des Init noch
    /// nicht, ist nichts zu unterbrechen, und der Aufrufer eskaliert.
    #[must_use]
    pub fn interrupt(&self, grace: Duration) -> bool {
        if self.try_wait().is_some() {
            return true;
        }
        let Some(child) = self.child_pid() else {
            return false;
        };
        let Some(pid) = Pid::from_raw(i32::try_from(child).unwrap_or(0)) else {
            return false;
        };
        // ESRCH heißt: die Gruppe ist schon weg, und dann ist auch die Sandbox
        // gleich weg; jeder andere Fehler wäre ein Recht, das wir bei einer
        // selbst gestarteten Sandbox haben. In beiden Fällen entscheidet
        // allein, ob der Prozess in der Frist endet.
        // `child_pid` wird von bwrap gereapt, nicht von uns: Zwischen dem Ende
        // des Init und dem Ende von bwrap koennte die Nummer theoretisch neu
        // vergeben sein. Das Fenster ist Millisekunden gross und braucht einen
        // vollen Umlauf des PID-Zaehlers; ein Signal an eine fremde Gruppe waere
        // dann SIGINT an einen Prozess desselben Nutzers, keine Eskalation.
        if kill_process_group(pid, Signal::INT).is_err() {
            return false;
        }
        self.shared.wait_exit(Some(grace)).is_some()
    }

    /// Wie [`SandboxHandle::kill`], mit eigener Frist zwischen den Signalen.
    pub fn terminate(&self, grace: Duration) {
        if self.try_wait().is_some() {
            return;
        }
        self.signal(Signal::TERM);
        if self.shared.wait_exit(Some(grace)).is_some() {
            return;
        }
        self.signal(Signal::KILL);
        // Nach SIGKILL bleibt nur das Einsammeln durch den wartenden Thread.
        let _ = self.shared.wait_exit(Some(KILL_GRACE));
    }

    fn signal(&self, signal: Signal) {
        // ESRCH heißt: schon weg, und ein anderer Fehler ist bei einem eigenen
        // Kind nicht möglich (EPERM bräuchte eine fremde UID). Ein Signal an
        // eine PID, die inzwischen ein anderer Prozess trägt, ist
        // ausgeschlossen, solange der wartende Thread das Kind noch nicht
        // eingesammelt hat; und hat er es, ist `try_wait` oben `Some`.
        if let Some(pid) = Pid::from_raw(i32::try_from(self.pid).unwrap_or(0)) {
            let _ = kill_process(pid, signal);
        }
    }

    /// Was der Shim bis jetzt gemeldet hat, ohne zu warten.
    #[must_use]
    pub fn report(&self) -> ReportSnapshot {
        lock(&self.shared.report).clone()
    }

    /// Wartet, bis der Bericht vollständig ist ([`ReportSnapshot::is_complete`]),
    /// die Pipe zu ist, die Sandbox beendet ist oder `timeout` um ist.
    #[must_use]
    pub fn wait_for_report(&self, timeout: Duration) -> ReportSnapshot {
        self.shared
            .wait_report(timeout, ReportSnapshot::is_complete)
    }

    /// Was `bwrap` über seine Status-Pipe gemeldet hat.
    #[must_use]
    pub fn status(&self) -> StatusSnapshot {
        lock(&self.shared.status).clone()
    }

    /// Die PID des Init-Prozesses der Sandbox auf dem Host, sobald `bwrap`
    /// sie gemeldet hat.
    #[must_use]
    pub fn child_pid(&self) -> Option<u32> {
        lock(&self.shared.status).child_pid
    }

    /// Die gesammelte Ausgabe, wenn die Sandbox mit
    /// [`crate::StdioMode::Capture`] lief und beendet ist; sonst `None`.
    #[must_use]
    pub fn output(&self) -> Option<CapturedOutput> {
        if !*lock(&self.shared.capturing) {
            return None;
        }
        self.try_wait()?;
        self.shared.join_readers();
        Some(CapturedOutput {
            stdout: lock(&self.shared.stdout).clone(),
            stderr: lock(&self.shared.stderr).clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::os::unix::process::ExitStatusExt;
    use std::process::ExitStatus;
    use std::sync::Arc;
    use std::time::Duration;

    use humanitl_core::ids::SandboxId;

    use super::{CAPTURE_MAX_BYTES, ReportSnapshot, SandboxHandle, Shared};
    use crate::bridge_env::ShimCheck;

    fn check(name: &str, ok: bool) -> ShimCheck {
        ShimCheck {
            name: name.to_owned(),
            ok,
            evidence: String::new(),
        }
    }

    #[test]
    fn a_report_is_complete_when_every_name_arrived() {
        let mut report = ReportSnapshot::default();
        assert!(!report.is_complete());
        for name in [
            "bridge_listening",
            "single_socket",
            "seccomp_applied",
            "families",
        ] {
            report.checks.push(check(name, true));
        }
        assert!(!report.is_complete());
        report.checks.push(check("no_interfaces", false));
        assert!(report.is_complete());
        assert_eq!(report.get("no_interfaces").map(|c| c.ok), Some(false));
        assert!(report.get("nothing").is_none());
    }

    #[test]
    fn waiting_for_the_report_ends_on_completion_or_timeout() {
        let shared = Arc::new(Shared::default());
        let handle = SandboxHandle::new(SandboxId::nil(), 0, String::new(), Arc::clone(&shared));
        let early = handle.wait_for_report(Duration::from_millis(60));
        assert!(early.checks.is_empty() && !early.closed);

        for name in [
            "bridge_listening",
            "single_socket",
            "seccomp_applied",
            "families",
            "no_interfaces",
        ] {
            shared.push_check(check(name, true));
        }
        shared.push_other_line();
        let full = handle.wait_for_report(Duration::from_secs(5));
        assert!(full.is_complete());
        assert_eq!(full.other_lines, 1);

        shared.close_report();
        assert!(handle.report().closed);
    }

    /// Ohne die PID des Sandbox-Init gibt es keine Prozessgruppe, an die die
    /// Bitte gehen könnte; dann eskaliert der Aufrufer.
    #[test]
    fn an_interrupt_without_a_known_init_pid_leaves_the_escalation_to_the_caller() {
        let shared = Arc::new(Shared::default());
        let handle = SandboxHandle::new(SandboxId::nil(), 0, String::new(), Arc::clone(&shared));
        assert!(handle.child_pid().is_none());
        assert!(!handle.interrupt(Duration::from_millis(50)));
    }

    /// Eine Sandbox, die schon beendet ist, ist nichts mehr zu unterbrechen.
    #[test]
    fn an_interrupt_after_the_end_is_nothing_to_do() {
        let shared = Arc::new(Shared::default());
        let handle = SandboxHandle::new(SandboxId::nil(), 0, String::new(), Arc::clone(&shared));
        shared.set_exit(ExitStatus::from_raw(0));
        assert!(handle.interrupt(Duration::from_millis(50)));
    }

    #[test]
    fn the_capture_is_capped() {
        let shared = Shared::default();
        shared.append_stdout(&vec![b'x'; CAPTURE_MAX_BYTES - 1]);
        shared.append_stdout(b"abc");
        assert_eq!(super::lock(&shared.stdout).len(), CAPTURE_MAX_BYTES);
        shared.append_stderr(b"bwrap: nope");
        assert_eq!(shared.stderr_excerpt(), "bwrap: nope");
    }

    #[test]
    fn output_is_none_without_capture_or_before_the_end() {
        let shared = Arc::new(Shared::default());
        let handle = SandboxHandle::new(SandboxId::nil(), 0, String::new(), Arc::clone(&shared));
        assert!(handle.output().is_none());
        shared.set_capturing();
        assert!(handle.output().is_none(), "not finished yet");
        assert!(handle.try_wait().is_none());
    }

    /// Der Befund hängt an der `exit-code`-Zeile: mit ihr lief der Befehl,
    /// ohne sie ist `bwrap` vorher gescheitert; ein Signal ist nie ein
    /// Startfehler.
    #[test]
    fn the_verdict_reads_the_exit_code_line() {
        let exit_1 = ExitStatus::from_raw(1 << 8);

        // Kein Exit-Code gemeldet, Pipe zu, bwrap endete mit 1: Startfehler
        // mit der gesammelten Fehlerausgabe.
        let shared = Shared::default();
        shared.set_capturing();
        shared.append_stderr(b"bwrap: Can't find source path /nope: No such file or directory\n");
        shared.close_status();
        let err = shared.verdict(exit_1).expect_err("no exit-code line");
        assert_eq!(err.code.as_str(), "SANDBOX_012");
        assert!(
            err.why.contains("before starting the command"),
            "{}",
            err.why
        );
        assert!(err.why.contains("/nope"), "{}", err.why);

        // Dieselbe Lage, aber die Fehlerausgabe ging ans geerbte stderr.
        let shared = Shared::default();
        shared.close_status();
        let err = shared.verdict(exit_1).expect_err("no exit-code line");
        assert!(err.why.contains("inherited stderr"), "{}", err.why);

        // Nutzer-Namensräume: der eigene Code mit dem Befehl zum Beheben.
        let shared = Shared::default();
        shared.set_capturing();
        shared.append_stderr(b"bwrap: setting up uid map: Permission denied\n");
        shared.close_status();
        let err = shared.verdict(exit_1).expect_err("userns");
        assert_eq!(err.code.as_str(), "SANDBOX_003");
        assert!(err.fix.is_some());

        // Exit-Code gemeldet: der Befehl lief und endete mit 1.
        let shared = Shared::default();
        shared.set_exit_code(1);
        shared.close_status();
        assert_eq!(shared.verdict(exit_1).expect("the command ran"), exit_1);

        // Ein Signal, ohne Zeile: kein Startfehler.
        let shared = Shared::default();
        shared.close_status();
        let killed = ExitStatus::from_raw(9);
        assert_eq!(shared.verdict(killed).expect("killed, not failed"), killed);
    }
}
