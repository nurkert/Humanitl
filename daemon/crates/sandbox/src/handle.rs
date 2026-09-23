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

use std::os::fd::{AsFd as _, OwnedFd};
use std::process::ExitStatus;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use humanitl_core::diagnostics::codes::{SANDBOX_012, TERM_002};
use humanitl_core::ids::SandboxId;
use humanitl_core::{Diagnostic, Severity};
use rustix::process::{Pid, Signal, kill_process, kill_process_group};
use rustix::termios::{Winsize, tcsetwinsize};

use crate::bridge_env::{CHECK_NAMES, ExecFailure, ShimCheck};
use crate::bwrap::{is_userns_failure, userns_diagnostic};
use crate::refusals::{RefusalLine, Refusals};

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

/// Wie viel vom Anfang der Terminalausgabe als Fehlerausgabe zählt.
///
/// Ein Pseudoterminal hat keine getrennte Fehlerausgabe: Was `bwrap` beim
/// Scheitern schreibt, kommt aus derselben Leitung wie alles andere. Ohne
/// diese Spiegelung fände der Befund von [`SandboxHandle::wait`] den Grund
/// nicht mehr: [`is_userns_failure`] hätte keine Quelle, und der Auszug in
/// `SANDBOX_012` bliebe leer. Gespiegelt wird nur der Anfang: Wenn `bwrap` scheitert,
/// scheitert es, bevor der Agent ein Zeichen geschrieben hat.
pub const PTY_MIRROR_BYTES: usize = 2048;

/// Wie eine Sandbox geendet hat, als [`SandboxHandle::terminate`] sie beendete.
///
/// Der Rückgabewert macht die Eskalation messbar statt behauptet: Ein
/// Aufrufer, der nur `SIGTERM` schickte und danach wartete, könnte bis
/// HUM-142 nicht unterscheiden, ob der Agent gegangen ist oder ob die Frist
/// verstrichen ist. Wer den Wert liest, sieht beides — und [`Termination::Stuck`]
/// ist der Fall, für den es einen Befund geben muss (`SANDBOX_029`), weil
/// danach ein Prozess übrig bleibt, den niemand mehr einsammelt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// Sie war schon beendet; es ging kein Signal hinaus.
    AlreadyEnded,
    /// `SIGTERM` hat gereicht, innerhalb der Frist.
    Term,
    /// Erst `SIGKILL` nach der Frist hat sie beendet.
    Kill,
    /// Auch nach `SIGKILL` und [`KILL_GRACE`] lag **kein Exit-Status** vor.
    ///
    /// Das ist weniger, als es klingt, und mehr als nichts. Der Status wird
    /// von dem Faden gesetzt, der die Sandbox gestartet hat, und zwar erst,
    /// nachdem er die Leser der Ausgabe eingesammelt hat. Ein Prozess kann
    /// deshalb längst eingesammelt sein, während ein Leser noch an einem
    /// Deskriptor hängt, der nicht schließt — dann steht hier `Stuck`, obwohl
    /// nichts mehr läuft. Wer daraus auf einen überlebenden Prozess schließen
    /// will, fragt [`SandboxHandle::process_alive`]; dieser Wert allein sagt
    /// es nicht.
    Stuck,
}

impl Termination {
    /// Ob ein Exit-Status vorliegt, die Sandbox also nachweislich beendet ist.
    #[must_use]
    pub const fn ended(self) -> bool {
        !matches!(self, Self::Stuck)
    }

    /// Ob es dazu mehr als `SIGTERM` gebraucht hat.
    #[must_use]
    pub const fn escalated(self) -> bool {
        matches!(self, Self::Kill | Self::Stuck)
    }

    /// Der Name für Protokoll und Befund.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyEnded => "already_ended",
            Self::Term => "sigterm",
            Self::Kill => "sigkill",
            Self::Stuck => "stuck",
        }
    }
}

impl std::fmt::Display for Termination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Welcher Strom der Sandbox ein Stück Ausgabe geschrieben hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputStream {
    /// Die Standardausgabe des Befehls in der Sandbox.
    Stdout,
    /// Seine Fehlerausgabe.
    Stderr,
}

/// Ein Stück Ausgabe, so wie es aus der Pipe kam.
///
/// Die Stücke folgen den Lesevorgängen und nicht den Zeilen: Wer Zeilen will,
/// setzt sie selbst zusammen. Der Weg über einen Kanal steht neben dem
/// gesammelten Puffer und nicht an seiner Stelle, weil beide verschiedene
/// Fragen beantworten — der Puffer „was kam insgesamt", bis
/// [`CAPTURE_MAX_BYTES`], der Kanal „was kommt gerade", ohne Grenze.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputChunk {
    /// Woher das Stück kam.
    pub stream: OutputStream,
    /// Die Bytes, ungefiltert und ungekürzt.
    pub bytes: Vec<u8>,
}

/// Wohin die Ausgabe zusätzlich zum Puffer geschickt wird.
///
/// Ein Sender, den [`crate::BwrapBackend::with_output_sink`] setzt, bekommt
/// jedes Stück, sobald es gelesen wurde. Bricht der Empfänger weg, wird das
/// Ergebnis verworfen: Die Sandbox darf nicht daran hängen, dass jemand
/// zuhört.
pub type OutputSink = std::sync::mpsc::Sender<OutputChunk>;

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
    /// Zeilen, die weder eine `CHECK`-Zeile noch eine über Verweigerungen
    /// waren.
    pub other_lines: usize,
    /// Was der Filter dem Agenten verweigert hat, gezählt (HUM-138).
    pub refusals: Refusals,
    /// Die Pipe ist zu: alle Schreibseiten sind geschlossen.
    pub closed: bool,
    /// Der Shim meldet ein gescheitertes `exec`: Der Agent ist nie gelaufen
    /// (HUM-137, [`crate::bridge_env::parse_exec_line`]).
    pub exec_failed: Option<ExecFailure>,
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
    /// Wohin jedes gelesene Stück zusätzlich geht, solange jemand zuhört.
    sink: Mutex<Option<OutputSink>>,
    /// Die Herrscherseite des Pseudoterminals, wenn die Sandbox an einem
    /// läuft ([`crate::StdioMode::Pty`]).
    ///
    /// Ein [`OnceLock`], weil es genau einen Start je Handle gibt und der
    /// Deskriptor vor der PID gesetzt wird: Wer das Handle hat, hat auch das
    /// Terminal. Gelesen wird er von genau einem Faden (dem Leser in
    /// `supervise`), geschrieben von dem Client, der schreiben darf.
    pty: OnceLock<OwnedFd>,
    /// Wie viele Bytes der Terminalausgabe schon als Fehlerausgabe zählen.
    mirrored: Mutex<usize>,
}

/// Das Feld `starttime` aus `/proc/<pid>/stat`, Feld 22 (Ticks seit dem
/// Systemstart); `None`, wenn sich die Zeile nicht lesen oder nicht deuten
/// lässt.
fn read_start_ticks(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    start_ticks_of(&stat)
}

/// Wie [`read_start_ticks`], aber über eine schon gelesene Zeile.
///
/// Gezählt wird hinter der **letzten** schließenden Klammer: Der Name des
/// Programms steht in Klammern und darf selbst Klammern und Leerzeichen
/// tragen. Danach ist Feld 3 der Zustand, und `starttime` ist Feld 22, also
/// das zwanzigste danach.
fn start_ticks_of(stat: &str) -> Option<u64> {
    fields_after_comm(stat)?.nth(19)?.parse().ok()
}

/// Die Felder hinter dem Namen des Programms, oder `None`, wenn die Zeile
/// keine `stat`-Zeile ist.
///
/// Gezählt wird hinter der **letzten** schließenden Klammer: Der Name steht in
/// Klammern und darf selbst Klammern und Leerzeichen tragen, und wer von vorn
/// zählt, zählt bei `(od d) ler)` falsch. Ohne Klammer wird nichts geraten.
fn fields_after_comm(stat: &str) -> Option<std::str::SplitWhitespace<'_>> {
    Some(stat.get(stat.rfind(')')? + 1..)?.split_whitespace())
}

/// Ob diese `stat`-Zeile einen laufenden Prozess beschreibt — und ob sie
/// überhaupt zu dem Prozess gehört, den wir meinen.
///
/// Zwei Fragen, eine Zeile:
///
/// - **Ist es noch derselbe Prozess?** Eine PID wird nach einem vollen Umlauf
///   des Zählers neu vergeben. Genau dieses Fenster steht offen, wenn
///   [`Termination::Stuck`] entsteht: Der Wirt hat das Kind da längst
///   eingesammelt, und nur der Status fehlt noch. Stimmt die Startzeit nicht
///   mit der überein, die beim Start gelesen wurde, gehört die Zeile einem
///   Fremden — unser Prozess ist weg.
/// - **Läuft er?** Ein Zombie (`Z`) zählt als beendet: kein Programm mehr, nur
///   noch ein Eintrag, den sein Elternprozess abholt.
fn alive_from_stat(stat: &str, started_at_ticks: Option<u64>) -> Option<bool> {
    let state = fields_after_comm(stat)?.next()?;
    // Wer einen Ausweis hat, muss ihn auch lesen können. Eine Zeile, die
    // abbricht, bevor Feld 22 kommt, beantwortet die Frage nach der Identität
    // nicht — und „keine Antwort" ist `None` und nicht „er lebt". Sonst stünde
    // hinter derselben Tür wieder der fremde Prozess, gegen den der Ausweis
    // eingeführt wurde.
    if let Some(expected) = started_at_ticks
        && start_ticks_of(stat)? != expected
    {
        return Some(false);
    }
    Some(state != "Z")
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

    /// Nimmt eine Zeile über Verweigerungen auf (HUM-138); weckt Wartende nur,
    /// wenn sich etwas geändert hat.
    pub(crate) fn push_refusal(&self, line: RefusalLine) {
        if lock(&self.report).refusals.apply(line) {
            self.report_changed.notify_all();
        }
    }

    pub(crate) fn wait_refusals(&self, after: u64, timeout: Duration) -> (Refusals, bool) {
        let deadline = Instant::now() + timeout;
        let mut report = lock(&self.report);
        loop {
            if report.refusals.generation > after || report.closed {
                return (report.refusals.clone(), report.closed);
            }
            let now = Instant::now();
            if now >= deadline {
                return (report.refusals.clone(), report.closed);
            }
            report = self
                .report_changed
                .wait_timeout(report, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
    }

    pub(crate) fn push_other_line(&self) {
        lock(&self.report).other_lines += 1;
        self.report_changed.notify_all();
    }

    pub(crate) fn set_exec_failed(&self, failure: ExecFailure) {
        lock(&self.report).exec_failed = Some(failure);
        self.report_changed.notify_all();
    }

    /// Wartet, bis die Berichts-Pipe zu ist, höchstens `timeout`. Anders als
    /// [`Shared::wait_report`] beendet das Ende der Sandbox das Warten nicht:
    /// Genau dann liest der Leser noch die letzten Zeilen.
    pub(crate) fn wait_report_closed(&self, timeout: Duration) -> ReportSnapshot {
        let deadline = Instant::now() + timeout;
        let mut report = lock(&self.report);
        loop {
            if report.closed {
                return report.clone();
            }
            let now = Instant::now();
            if now >= deadline {
                return report.clone();
            }
            report = self
                .report_changed
                .wait_timeout(report, deadline - now)
                .unwrap_or_else(PoisonError::into_inner)
                .0;
        }
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
        self.tee(OutputStream::Stdout, chunk);
    }

    pub(crate) fn append_stderr(&self, chunk: &[u8]) {
        append_capped(&mut lock(&self.stderr), chunk);
        self.tee(OutputStream::Stderr, chunk);
    }

    /// Ein Stück aus dem Pseudoterminal: ein Strom, und sein Anfang zählt
    /// zusätzlich als Fehlerausgabe.
    ///
    /// Die Spiegelung geht **nicht** über [`Shared::append_stderr`]: Sie füllt
    /// nur den Puffer, aus dem der Startbefund zitiert, und schickt kein
    /// zweites Stück an den Zuhörer. Wer mitliest, bekäme sonst jedes Byte des
    /// Anfangs doppelt, einmal als Ausgabe und einmal als Fehlerausgabe.
    pub(crate) fn append_pty(&self, chunk: &[u8]) {
        {
            let mut mirrored = lock(&self.mirrored);
            if *mirrored < PTY_MIRROR_BYTES {
                let room = PTY_MIRROR_BYTES - *mirrored;
                let cut = chunk.len().min(room);
                append_capped(&mut lock(&self.stderr), &chunk[..cut]);
                *mirrored += cut;
            }
        }
        append_capped(&mut lock(&self.stdout), chunk);
        self.tee(OutputStream::Stdout, chunk);
    }

    /// Legt die Herrscherseite des Pseudoterminals ab; einmal je Start.
    pub(crate) fn set_pty(&self, master: OwnedFd) {
        let _ = self.pty.set(master);
    }

    /// Schickt ein Stück an den Zuhörer, falls einer da ist.
    ///
    /// Ungekürzt, anders als der Puffer: Wer mitliest, schreibt weiter, und
    /// [`CAPTURE_MAX_BYTES`] begrenzt nur, was der Daemon aufhebt. Ein
    /// abgebrochener Kanal wird stillschweigend fallen gelassen — der Leser
    /// dieses Threads darf nie an einem Empfänger hängen bleiben.
    fn tee(&self, stream: OutputStream, chunk: &[u8]) {
        let sink = lock(&self.sink);
        if let Some(sink) = sink.as_ref() {
            let _ = sink.send(OutputChunk {
                stream,
                bytes: chunk.to_vec(),
            });
        }
    }

    pub(crate) fn set_sink(&self, sink: OutputSink) {
        *lock(&self.sink) = Some(sink);
    }

    /// Lässt den Zuhörer los, damit sein Kanal endet.
    ///
    /// Wird gerufen, wenn die Leser eingesammelt sind: Ab dann kommt nichts
    /// mehr, und wer auf das Ende des Kanals wartet, soll es sehen.
    pub(crate) fn clear_sink(&self) {
        *lock(&self.sink) = None;
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

/// Der Befund, wenn das Terminal einer Sandbox nicht mehr antwortet.
fn terminal_gone(why: &str) -> Diagnostic {
    Diagnostic::builder(TERM_002, Severity::Error)
        .why(why.to_owned())
        .build()
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
    /// Die Startzeit des Prozesses, wie `/proc/<pid>/stat` sie beim Start
    /// gemeldet hat (Feld 22, in Ticks seit dem Systemstart).
    ///
    /// Sie ist der Ausweis der PID: Eine Nummer wird nach einem vollen Umlauf
    /// des Zählers neu vergeben, und dann trägt sie einen fremden Prozess mit
    /// einer anderen Startzeit. `None`, wenn sie sich nicht lesen ließ; dann
    /// gilt die Auskunft ohne diesen Ausweis.
    started_at_ticks: Option<u64>,
    shared: Arc<Shared>,
}

impl SandboxHandle {
    pub(crate) fn new(id: SandboxId, pid: u32, argv_display: String, shared: Arc<Shared>) -> Self {
        Self {
            id,
            pid,
            argv_display,
            // Jetzt gelesen und nicht später: Der Prozess läuft gerade, und
            // später ist die Nummer vielleicht nicht mehr seine.
            started_at_ticks: read_start_ticks(pid),
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
    /// Kehrt zurück, wenn der Prozess weg ist, und sagt, was es dazu gebraucht
    /// hat. `bwrap` reicht `SIGTERM` nicht an das Kind durch, aber mit
    /// `--die-with-parent` endet mit `bwrap` der ganze PID-Namensraum;
    /// `SIGKILL` an `bwrap` beendet deshalb auch den Agenten.
    ///
    /// Der Rückgabewert darf ignoriert werden: Wer nur beenden will, ruft
    /// weiter `handle.kill();`. Ein `#[must_use]` zwänge jede dieser Stellen
    /// zu einem `let _`, ohne dass dort jemand die Auskunft braucht.
    #[allow(clippy::must_use_candidate)]
    pub fn kill(&self) -> Termination {
        self.terminate(KILL_GRACE)
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
    /// - Das Init des PID-Namensraums ist der Shim (`--as-pid-1`, HUM-203;
    ///   [`SandboxHandle::child_pid`]). Er ignoriert `SIGINT`, und ein Signal
    ///   ohne Handler an ein Namensraum-Init verwirft der Kernel ohnehin. Das
    ///   Init trägt aber wegen `--new-session` die Sitzung und die
    ///   Prozessgruppe der Sandbox, und der Agent hängt darin.
    ///   `kill(-child_pid, SIGINT)` erreicht deshalb genau den Agenten, ohne
    ///   dass der Shim es weiterreichen muss.
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
    ///
    /// Die Frist ist der Punkt: Ein Agent, der `SIGTERM` selbst abfängt — ein
    /// Vollbild-TUI tut das —, hält den Abschied sonst für immer auf. Nach
    /// `grace` folgt deshalb `SIGKILL`, und der Rückgabewert sagt, welcher der
    /// beiden Wege es war (HUM-142). Die obere Schranke ist
    /// `grace` + [`KILL_GRACE`].
    ///
    /// Der Rückgabewert darf ignoriert werden, wie bei [`SandboxHandle::kill`];
    /// er ist die Messung für den, der sie nennen muss.
    #[allow(clippy::must_use_candidate)]
    pub fn terminate(&self, grace: Duration) -> Termination {
        if self.try_wait().is_some() {
            return Termination::AlreadyEnded;
        }
        self.signal(Signal::TERM);
        if self.shared.wait_exit(Some(grace)).is_some() {
            return Termination::Term;
        }
        self.signal(Signal::KILL);
        // Nach SIGKILL bleibt nur das Einsammeln durch den wartenden Thread.
        if self.shared.wait_exit(Some(KILL_GRACE)).is_some() {
            Termination::Kill
        } else {
            Termination::Stuck
        }
    }

    fn signal(&self, signal: Signal) {
        // ESRCH heißt: schon weg, und ein anderer Fehler ist bei einem eigenen
        // Kind nicht möglich (EPERM bräuchte eine fremde UID).
        //
        // **Eine neu vergebene Nummer ist nicht ausgeschlossen, nur sehr
        // unwahrscheinlich.** Der wartende Faden sammelt das Kind ein und setzt
        // den Exit-Status erst danach; dazwischen steht `try_wait` noch auf
        // `None`, während die Nummer schon frei ist. Wer sie danach bekäme,
        // müsste den ganzen PID-Zähler umlaufen haben, und das Signal ginge
        // dann an einen Prozess desselben Nutzers. Dasselbe Fenster ist es,
        // gegen das [`alive_from_stat`] die Startzeit prüft — dort kostet es
        // nichts, hier ließe es sich nur mit einem pidfd schließen.
        if let Some(pid) = Pid::from_raw(i32::try_from(self.pid).unwrap_or(0)) {
            let _ = kill_process(pid, signal);
        }
    }

    /// Was der Shim bis jetzt gemeldet hat, ohne zu warten.
    #[must_use]
    pub fn report(&self) -> ReportSnapshot {
        lock(&self.shared.report).clone()
    }

    /// Der ganze Bericht einer beendeten Sandbox: wartet höchstens `timeout`,
    /// bis die Pipe zu ist, damit auch die letzte Zeile gelesen ist.
    ///
    /// Für die Frage, ob der Shim ein gescheitertes `exec` gemeldet hat
    /// ([`ReportSnapshot::exec_failed`]); die Zeile kommt nach allen `CHECK`,
    /// und wer nur bis zu den fünf Namen wartet, sieht sie nicht.
    #[must_use]
    pub fn report_after_exit(&self, timeout: Duration) -> ReportSnapshot {
        self.shared.wait_report_closed(timeout)
    }

    /// Wartet, bis der Bericht vollständig ist ([`ReportSnapshot::is_complete`]),
    /// die Pipe zu ist, die Sandbox beendet ist oder `timeout` um ist.
    #[must_use]
    pub fn wait_for_report(&self, timeout: Duration) -> ReportSnapshot {
        self.shared
            .wait_report(timeout, ReportSnapshot::is_complete)
    }

    /// Wartet auf einen neueren Stand der Verweigerungen als `after`
    /// ([`Refusals::generation`]), höchstens `timeout` (HUM-138).
    ///
    /// Endet auch, wenn die Pipe des Berichts zu ist: Dann kommt nichts mehr,
    /// und der Stand ist der letzte. Das Ende der Sandbox allein beendet das
    /// Warten nicht — die letzten Zeilen schreibt der Shim erst, wenn der Agent
    /// gegangen ist, und der Leser ist ihnen dann womöglich noch hinterher.
    /// Zurück kommt der Stand und ob die Pipe zu ist.
    #[must_use]
    pub fn wait_refusals(&self, after: u64, timeout: Duration) -> (Refusals, bool) {
        self.shared.wait_refusals(after, timeout)
    }

    /// Was `bwrap` über seine Status-Pipe gemeldet hat.
    #[must_use]
    pub fn status(&self) -> StatusSnapshot {
        lock(&self.shared.status).clone()
    }

    /// Ob der Prozess der Sandbox auf dem Wirt noch läuft.
    ///
    /// `None`, wenn sich das nicht sagen lässt (kein `/proc`). Ein Zombie
    /// zählt als beendet: Er hat kein Programm mehr, nur noch einen Eintrag,
    /// den sein Elternprozess abholt.
    ///
    /// Gefragt wird das nach [`Termination::Stuck`], und nur dort: Ein
    /// fehlender Exit-Status heißt nicht, dass der Prozess lebt (siehe
    /// [`Termination::Stuck`]), und eine Meldung darüber, dass ein Prozess
    /// stehen geblieben sei, darf nicht auf einer Vermutung stehen.
    #[must_use]
    pub fn process_alive(&self) -> Option<bool> {
        match std::fs::read_to_string(format!("/proc/{}/stat", self.pid)) {
            Ok(stat) => alive_from_stat(&stat, self.started_at_ticks),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Some(false),
            Err(_) => None,
        }
    }

    /// Die PID des Init-Prozesses der Sandbox auf dem Host, sobald `bwrap`
    /// sie gemeldet hat.
    #[must_use]
    pub fn child_pid(&self) -> Option<u32> {
        lock(&self.shared.status).child_pid
    }

    /// Die Herrscherseite des Pseudoterminals, wenn die Sandbox an einem läuft.
    ///
    /// `None` in jedem anderen [`crate::StdioMode`]. Der Deskriptor gehört dem
    /// Handle; wer ihn braucht, leiht ihn sich oder verdoppelt ihn. Gelesen
    /// wird er bereits vom Leser dieser Crate — ein zweiter Leser bekäme
    /// zufällige Hälften des Stroms —, geschrieben wird über
    /// [`SandboxHandle::write_input`].
    #[must_use]
    pub fn pty_master(&self) -> Option<&OwnedFd> {
        self.shared.pty.get()
    }

    /// Schreibt Bytes in die Eingabe des Agenten.
    ///
    /// Das ist die Tastatur des Menschen, nicht mehr: Der Agent liest sie als
    /// Terminaleingabe. `Ctrl+C` erreicht ihn als Byte `0x03` und nicht als
    /// `SIGINT`, denn die Sandbox läuft mit `--new-session` und hat kein
    /// steuerndes Terminal (`docs/THREAT-MODEL.md`, Absatz zu `TIOCSTI`); wer
    /// wirklich unterbrechen will, nimmt [`SandboxHandle::interrupt`].
    ///
    /// Der Aufruf blockiert, wenn der Agent nicht liest und der Puffer des
    /// Terminals voll ist. Er gehört deshalb auf einen Faden, der blockieren
    /// darf.
    ///
    /// # Errors
    ///
    /// `TERM_002`, wenn diese Sandbox kein Pseudoterminal hat oder das
    /// Schreiben fehlschlägt — beendet der Agent sich, während der Mensch
    /// tippt, ist das `EIO`.
    pub fn write_input(&self, bytes: &[u8]) -> Result<(), Diagnostic> {
        let master = self.pty_or_refuse("write to the terminal of")?;
        let mut rest = bytes;
        while !rest.is_empty() {
            match rustix::io::write(master.as_fd(), rest) {
                Ok(0) => {
                    return Err(terminal_gone("the terminal accepted no more input"));
                }
                Ok(written) => rest = &rest[written.min(rest.len())..],
                Err(rustix::io::Errno::INTR) => {}
                Err(err) => {
                    return Err(terminal_gone(&format!(
                        "cannot write to the terminal of the sandbox: {err}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Setzt die Geometrie des Terminals und sagt dem Agenten Bescheid.
    ///
    /// Zwei Schritte, und der zweite ist nicht selbstverständlich:
    /// `tcsetwinsize` trägt die neue Größe ein, aber der Kernel schickt das
    /// `SIGWINCH` nur an die Vordergrund-Prozessgruppe des *steuernden*
    /// Terminals — und die Sandbox hat mit `--new-session` keines. Ohne das
    /// Signal fragt kein Vollbild-TUI die neue Größe ab und zeichnet weiter
    /// im alten Raster. Das Signal geht deshalb an die Prozessgruppe des
    /// Sandbox-Init, genau wie bei [`SandboxHandle::interrupt`] und aus
    /// demselben Grund: Ein Signal an die `bwrap`-PID allein wäre eines an
    /// das Init des PID-Namensraums, und das verwirft der Kernel.
    ///
    /// # Errors
    ///
    /// `TERM_002`, wenn diese Sandbox kein Pseudoterminal hat oder der Kernel
    /// die Größe nicht annimmt. Ein Signal, das niemanden mehr erreicht, ist
    /// kein Fehler: Die Größe steht dann trotzdem, und der nächste Prozess
    /// liest sie.
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Diagnostic> {
        let master = self.pty_or_refuse("resize the terminal of")?;
        let size = Winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        tcsetwinsize(master.as_fd(), size).map_err(|err| {
            terminal_gone(&format!(
                "cannot set the terminal of the sandbox to {cols}x{rows}: {err}"
            ))
        })?;
        if let Some(pid) = self
            .child_pid()
            .and_then(|child| i32::try_from(child).ok())
            .and_then(Pid::from_raw)
        {
            let _ = kill_process_group(pid, Signal::WINCH);
        }
        Ok(())
    }

    /// Das Terminal dieser Sandbox, oder der Befund, dass sie keines hat.
    fn pty_or_refuse(&self, what: &str) -> Result<&OwnedFd, Diagnostic> {
        self.pty_master().ok_or_else(|| {
            Diagnostic::builder(TERM_002, Severity::Error)
                .why(format!(
                    "cannot {what} sandbox {}: it runs without a terminal",
                    self.id
                ))
                .build()
        })
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

    use super::{
        CAPTURE_MAX_BYTES, PTY_MIRROR_BYTES, ReportSnapshot, SandboxHandle, Shared, Termination,
        alive_from_stat, start_ticks_of,
    };
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

    /// Die Startdiagnostik überlebt das Pseudoterminal.
    ///
    /// Am PTY gibt es keine getrennte Fehlerausgabe: Was `bwrap` beim
    /// Scheitern schreibt, kommt aus derselben Leitung wie die Ausgabe des
    /// Agenten. Ohne die Spiegelung der ersten [`PTY_MIRROR_BYTES`] fände
    /// [`Shared::verdict`] nichts, `is_userns_failure` verlöre seine Quelle,
    /// und aus `SANDBOX_003` mit Behebungsvorschlag würde ein `SANDBOX_012`
    /// ohne Grund.
    #[test]
    fn the_start_diagnostic_survives_the_pty() {
        let exit_1 = ExitStatus::from_raw(1 << 8);

        let shared = Shared::default();
        shared.set_capturing();
        shared.append_pty(b"bwrap: setting up uid map: Permission denied\r\n");
        shared.close_status();
        let err = shared.verdict(exit_1).expect_err("userns");
        assert_eq!(err.code.as_str(), "SANDBOX_003");
        assert!(err.fix.is_some(), "and it still says how to fix it");

        // Ein anderer Startfehler zitiert die Meldung.
        let shared = Shared::default();
        shared.set_capturing();
        shared.append_pty(b"bwrap: Can't find source path /nope: No such file or directory\r\n");
        shared.close_status();
        let err = shared.verdict(exit_1).expect_err("no exit-code line");
        assert_eq!(err.code.as_str(), "SANDBOX_012");
        assert!(err.why.contains("/nope"), "{}", err.why);
        assert!(
            !err.why.contains("inherited stderr"),
            "a terminal is not the inherited stderr: {}",
            err.why
        );
    }

    /// Gespiegelt wird der Anfang, nicht der Strom.
    ///
    /// Sonst zitierte ein Befund irgendwann den Agenten statt `bwrap`, und der
    /// Puffer der Fehlerausgabe trüge dieselben Bytes ein zweites Mal.
    #[test]
    fn the_mirror_stops_after_the_first_bytes() {
        let shared = Shared::default();
        shared.set_capturing();
        shared.append_pty(&vec![b'x'; PTY_MIRROR_BYTES]);
        shared.append_pty(b"the agent writes on");
        assert_eq!(super::lock(&shared.stderr).len(), PTY_MIRROR_BYTES);
        assert_eq!(
            super::lock(&shared.stdout).len(),
            PTY_MIRROR_BYTES + "the agent writes on".len(),
            "the output itself is not cut"
        );
    }

    /// Ein Terminal hat einen Strom, und der Zuhörer bekommt jedes Stück
    /// genau einmal.
    #[test]
    fn the_terminal_has_one_stream_and_no_echo() {
        let shared = Shared::default();
        let (tx, rx) = std::sync::mpsc::channel();
        shared.set_sink(tx);
        shared.append_pty(b"hello");
        shared.clear_sink();
        let chunks: Vec<super::OutputChunk> = rx.iter().collect();
        assert_eq!(chunks.len(), 1, "one chunk, not one per buffer: {chunks:?}");
        assert_eq!(chunks[0].stream, super::OutputStream::Stdout);
        assert_eq!(chunks[0].bytes, b"hello");
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

    /// Ein Kind für diese Tests, der Faden, der es einsammelt, und das
    /// Aufräumen, das auch eine gescheiterte Zusicherung überlebt.
    ///
    /// Dasselbe Gespann wie im Launcher: Ein Faden wartet auf das Kind und
    /// legt den Status in den geteilten Zustand; das Handle sieht nur diesen
    /// Zustand. Ohne den Faden bliebe [`Shared::wait_exit`] für immer stehen,
    /// und der Test prüfte nur seine eigene Frist.
    ///
    /// **Aufgeräumt wird in `Drop` und nicht in der letzten Zeile des Tests.**
    /// Am 2026-09-13 hat eine Mutationsprobe genau hier ein Kind stehen
    /// lassen: Die Zusicherung schlug an, der Test brach vor seiner eigenen
    /// Aufräumzeile ab, und das Kind lief weiter. Es hatte den Deskriptor der
    /// Sperre geerbt, unter der der Testlauf lief, und hielt sie damit für
    /// jeden anderen Lauf auf dieser Maschine.
    struct Deaf {
        pid: u32,
        shared: Arc<Shared>,
        waiter: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for Deaf {
        fn drop(&mut self) {
            if let Some(pid) = rustix::process::Pid::from_raw(i32::try_from(self.pid).unwrap_or(0))
            {
                let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
            }
            if let Some(waiter) = self.waiter.take() {
                let _ = waiter.join();
            }
        }
    }

    /// Ein Kind, das `SIGTERM` abfängt.
    fn stubborn_child() -> Deaf {
        deaf_child("trap '' TERM; while :; do sleep 0.05; done")
    }

    /// Wie [`stubborn_child`], aber mit dem Skript als Argument.
    fn deaf_child(script: &str) -> Deaf {
        let mut child = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(script)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("/bin/sh starts");
        let pid = child.id();
        let shared = Arc::new(Shared::default());
        let waiter = {
            let shared = Arc::clone(&shared);
            std::thread::spawn(move || {
                let status = child.wait().unwrap_or_default();
                shared.set_exit(status);
            })
        };
        Deaf {
            pid,
            shared,
            waiter: Some(waiter),
        }
    }

    /// Wartet, bis das Kind wirklich läuft: `sh` hat den Trap gesetzt, sobald
    /// die Marke da ist.
    fn settle() {
        std::thread::sleep(Duration::from_millis(150));
    }

    /// Ein Agent, der `SIGTERM` abfängt, ist nach der Frist trotzdem beendet
    /// (HUM-142).
    ///
    /// Gemessen an drei Dingen, damit kein Zufall als Erfolg durchgeht: der
    /// gemeldete Weg ist `SIGKILL`, der Status trägt Signal 9, und die Zeit
    /// liegt zwischen der Frist (ohne die Eskalation wäre sie nie zu Ende) und
    /// der Frist plus einer Sekunde.
    #[test]
    fn a_child_that_ignores_sigterm_is_killed_after_the_grace() {
        let child = stubborn_child();
        settle();
        let handle = SandboxHandle::new(
            SandboxId::nil(),
            child.pid,
            String::new(),
            Arc::clone(&child.shared),
        );
        let grace = Duration::from_millis(300);

        let started = std::time::Instant::now();
        let ended = handle.terminate(grace);
        let elapsed = started.elapsed();
        let status = handle.try_wait();

        assert_eq!(
            ended,
            Termination::Kill,
            "SIGTERM is ignored, so only SIGKILL can end it (took {elapsed:?})"
        );
        assert!(ended.escalated(), "{ended} counts as an escalation");
        assert!(ended.ended(), "{ended} means the process is gone");
        assert_eq!(
            status.and_then(|status| status.signal()),
            Some(9),
            "the child is reaped and carries SIGKILL"
        );
        assert!(
            elapsed >= grace,
            "the grace belongs to the agent; it was cut short after {elapsed:?}"
        );
        assert!(
            elapsed < grace + Duration::from_secs(1),
            "after the grace the kill follows at once, not after {elapsed:?}"
        );
    }

    /// Eine Zeile aus `/proc/<pid>/stat`, wie der Kern sie schreibt.
    ///
    /// Der Name in Klammern trägt hier selbst eine Klammer und ein
    /// Leerzeichen: Genau daran scheitert jeder Leser, der von vorn zählt.
    fn stat_line(state: &str, start_ticks: u64) -> String {
        let mut fields = vec!["1".to_owned(); 50];
        fields[0] = "4711".to_owned();
        fields[1] = "(od d) ler)".to_owned();
        fields[2] = state.to_owned();
        fields[21] = start_ticks.to_string();
        format!("{}\n", fields.join(" "))
    }

    /// Die Startzeit ist der Ausweis der PID: Ohne sie hielte der Abschied
    /// einen fremden Prozess für den eigenen (HUM-142).
    ///
    /// Das Fenster ist echt, wenn auch schmal: `Stuck` entsteht genau dann,
    /// wenn der Wirt das Kind schon eingesammelt hat und nur der Status fehlt
    /// — die Nummer ist da bereits frei. Wer sie nach einem vollen Umlauf des
    /// Zählers bekommt, bekäme sonst ein blockierendes `SANDBOX_029` und den
    /// Rat, sich seinen Zustand anzusehen.
    #[test]
    fn a_reused_pid_is_not_our_process() {
        let ours = stat_line("S", 8_800);

        assert_eq!(
            alive_from_stat(&ours, Some(8_800)),
            Some(true),
            "same start time, running: that is our process"
        );
        assert_eq!(
            alive_from_stat(&ours, Some(9_999)),
            Some(false),
            "another start time under the same number is a stranger, so ours is gone"
        );
        assert_eq!(
            alive_from_stat(&ours, None),
            Some(true),
            "without a recorded start time the state alone decides"
        );
        assert_eq!(
            alive_from_stat(&stat_line("Z", 8_800), Some(8_800)),
            Some(false),
            "a zombie has no program any more"
        );
        assert_eq!(
            start_ticks_of(&ours),
            Some(8_800),
            "the start time is read behind the last closing bracket, not from the front"
        );
        assert_eq!(
            alive_from_stat("nonsense", None),
            None,
            "no field, no answer"
        );

        // Eine Zeile, die vor Feld 22 abbricht, sagt nichts über die
        // Identität — und dann sagt auch diese Funktion nichts. Ohne diesen
        // Zweig stünde hier `Some(true)`, also genau die Behauptung, gegen die
        // der Ausweis eingeführt wurde.
        let truncated = "4711 (od d) ler) S 1 1 1 1 1 1 1\n";
        assert_eq!(
            alive_from_stat(truncated, Some(8_800)),
            None,
            "a line that stops before field 22 cannot confirm the identity"
        );
        assert_eq!(
            alive_from_stat(truncated, None),
            Some(true),
            "without a recorded start time there is nothing to confirm"
        );
    }

    /// Die Frage nach dem Prozess beantwortet drei Lagen, und ein Zombie zählt
    /// als beendet (HUM-142).
    ///
    /// Daran hängt die Stufe von `SANDBOX_029`: Ohne diese Frage müsste ein
    /// ausbleibender Exit-Status als überlebender Prozess gelten, und das wäre
    /// eine Behauptung ohne Beleg.
    #[test]
    fn a_zombie_counts_as_ended_and_a_running_child_does_not() {
        let child = stubborn_child();
        settle();
        let handle = SandboxHandle::new(
            SandboxId::nil(),
            child.pid,
            String::new(),
            Arc::clone(&child.shared),
        );
        assert_eq!(handle.process_alive(), Some(true), "the child runs");

        // Ein Kind, das endet und nicht eingesammelt wird, ist ein Zombie: Es
        // steht in `/proc`, aber es läuft nichts mehr.
        let mut short = std::process::Command::new("/bin/sh")
            .arg("-c")
            .arg("exit 0")
            .spawn()
            .expect("/bin/sh starts");
        let zombie = SandboxHandle::new(
            SandboxId::nil(),
            short.id(),
            String::new(),
            Arc::new(Shared::default()),
        );
        settle();
        assert_eq!(
            zombie.process_alive(),
            Some(false),
            "a zombie has no program any more"
        );
        short.wait().expect("the zombie is reaped");

        // Und eine PID, die es nicht gibt, ist ebenso wenig da. Sie kann in
        // der Zwischenzeit neu vergeben werden; deshalb erst nach dem
        // Einsammeln, und deshalb mit einer Nummer, die der Kern nicht
        // vergibt.
        let gone = SandboxHandle::new(
            SandboxId::nil(),
            0,
            String::new(),
            Arc::new(Shared::default()),
        );
        assert_eq!(gone.process_alive(), Some(false), "pid 0 is no process");
    }

    /// Ein Agent, der auf `SIGTERM` selbst geht, bekommt kein `SIGKILL` — die
    /// Frist gehört ihm, und der Rückgabewert sagt es.
    #[test]
    fn a_child_that_obeys_sigterm_keeps_its_grace() {
        let child = deaf_child("while :; do sleep 0.05; done");
        settle();
        let handle = SandboxHandle::new(
            SandboxId::nil(),
            child.pid,
            String::new(),
            Arc::clone(&child.shared),
        );
        let grace = Duration::from_secs(5);

        let started = std::time::Instant::now();
        let ended = handle.terminate(grace);
        let elapsed = started.elapsed();
        let status = handle.try_wait();

        assert_eq!(ended, Termination::Term, "SIGTERM was enough");
        assert!(!ended.escalated(), "{ended} is no escalation");
        assert_eq!(
            status.and_then(|status| status.signal()),
            Some(15),
            "the child carries SIGTERM, not SIGKILL"
        );
        assert!(
            elapsed < grace,
            "an agent that goes does not spend the grace: {elapsed:?}"
        );

        // Ein zweites Mal ist nichts mehr zu tun, und das ist kein Signal.
        assert_eq!(handle.terminate(grace), Termination::AlreadyEnded);
    }
}
