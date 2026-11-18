//! Die Suche der Backends: nebeneinander, mit Frist und kurz gemerkt
//! (HUM-502).
//!
//! `Doctor` und `SandboxBackends` fragen jedes Backend wie die Suche vor
//! einem Start. Unter der Container-Engine sind das bis zu drei Programme mit
//! je 30 s Frist, unter der Kernel-Stufe bis zu fünf Fragen nach der Hilfe
//! mit je 5 s. Nacheinander und bei jedem Aufruf neu reichte das nie für die
//! Frist der Oberfläche (5 s je Aufruf).
//!
//! # Wie die Suche läuft
//!
//! - **Nebeneinander.** Jedes Backend sucht in einem eigenen Thread. Mehr als
//!   eine Suche je Backend und Eingabe läuft nie: Wer fragt, während sie
//!   läuft, wartet auf dieselbe. Die Zahl der Threads ist damit durch die
//!   Zahl der Backends je Konfiguration begrenzt, und jede Suche endet an den
//!   Fristen ihrer Programme.
//! - **Kurz gemerkt.** Ein Ergebnis gilt [`SEARCH_TTL`] lang, und nur,
//!   solange sich keine der Dateien geändert hat, von denen die Suche abhängt
//!   (Pfad, Gerät, Inode, Größe, `mtime`, `ctime`). Welche das sind, weiß der
//!   Adapter (`crate::backend::search_files`); ein Programm, das neu im
//!   `PATH` auftaucht, zählt als Änderung.
//! - **Mit Frist.** Wer eine Frist nennt, bekommt nach ihr, was fertig ist;
//!   eine Suche, die noch läuft, steht als „wird geprüft" da
//!   (`Waited::Pending`) und läuft im Hintergrund zu Ende. Die nächste
//!   Antwort nimmt ihr Ergebnis. Ohne Frist wartet der Aufrufer auf alles,
//!   wie die Kommandozeile.
//!
//! Gemerkt wird nur, was das System über ein Backend sagt. Untergrenze,
//! Zulassung, Profil und die Obergrenzen des Rechners prüft jede Antwort neu;
//! das kostet nichts und hängt an der Konfiguration, nicht am System.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

use humanitl_config::{Env, Paths, SandboxRef};
use humanitl_core::diagnostics::codes::DOCTOR_012;
use humanitl_core::{Diagnostic, Severity};

use crate::backend::Backend;
use crate::doctor::backends::DoctorSearch;

/// So lange gilt ein Ergebnis der Suche, wenn sich keine seiner Dateien
/// ändert.
///
/// Kurz genug, dass ein Mensch, der etwas außerhalb dieser Dateien ändert
/// (etwa den Dienstverwalter startet), das Ergebnis nach einer halben Minute
/// sieht; lang genug, dass die Oberfläche beim Öffnen von zwei Bildschirmen
/// nicht zweimal sucht.
pub const SEARCH_TTL: Duration = Duration::from_secs(30);

/// Der Suchpfad, wenn die Umgebung keinen hat; derselbe wie bei der Suche.
const DEFAULT_PATH: &str = "/usr/local/bin:/usr/bin:/bin";

/// Das Gedächtnis der Suche eines Daemons: je Backend und Eingabe die letzte
/// Suche, laufend oder fertig.
///
/// Billig zu klonen; alle Klone teilen dasselbe Gedächtnis. Der Daemon hält
/// eines für `Doctor` und `SandboxBackends` zusammen, damit beide dieselbe
/// Suche teilen.
#[derive(Debug, Clone)]
pub struct BackendSearches {
    /// Die Suche vor jedem Start, je Backend ([`crate::backend::detect_in_build`]).
    pub(crate) detected: Memo<Result<Backend, Diagnostic>>,
    /// Was der Doctor darüber hinaus fragt: die Voraussetzungen einzeln.
    pub(crate) doctor: Memo<DoctorSearch>,
}

impl BackendSearches {
    /// Ein leeres Gedächtnis mit [`SEARCH_TTL`].
    #[must_use]
    pub fn new() -> Self {
        Self::with_ttl(SEARCH_TTL)
    }

    /// Ein leeres Gedächtnis, dessen Ergebnisse `ttl` lang gelten.
    #[must_use]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            detected: Memo::new(ttl),
            doctor: Memo::new(ttl),
        }
    }

    /// Startet die Suche vor jedem Start für dieses Backend, oder schließt
    /// sich der an, die läuft oder gilt.
    pub(crate) fn detect(
        &self,
        name: &'static str,
        sandbox: &SandboxRef,
        paths: &Paths,
    ) -> Ticket<Result<Backend, Diagnostic>> {
        let (owned_sandbox, owned_paths) = (sandbox.clone(), paths.clone());
        self.detected.start(
            search_key(name, sandbox, paths),
            &crate::backend::search_files(name, sandbox, paths),
            move || {
                Some(crate::backend::detect_in_build(
                    name,
                    owned_paths,
                    &owned_sandbox,
                ))
            },
        )
    }
}

impl BackendSearches {
    /// Das Backend aus der Suche vor jedem Start, über dieses Gedächtnis und
    /// höchstens bis `deadline` (HUM-536, der Startvorschlag): eine Suche,
    /// die läuft oder noch gilt, wird mitbenutzt, keine startet doppelt.
    ///
    /// # Errors
    ///
    /// Der Befund der Suche (etwa `SANDBOX_045` ohne `/dev/kvm`), oder
    /// `DOCTOR_012`, wenn sie bei `deadline` noch läuft.
    pub fn backend_within(
        &self,
        name: &'static str,
        sandbox: &SandboxRef,
        paths: &Paths,
        deadline: Option<Instant>,
    ) -> Result<Backend, Diagnostic> {
        match self.detect(name, sandbox, paths).wait(deadline) {
            Waited::Done(found) => found,
            Waited::Pending(running) => Err(still_searching(
                name,
                running,
                &["humanitl", "projects", "proposal"],
            )),
        }
    }

    /// Verwirft, was die Suche über dieses Backend weiß, für einen
    /// ausdrücklichen Wunsch wie den Probestart des Doctors: Eine Abhilfe, die
    /// der Mensch eben getroffen hat, zählt sofort.
    pub(crate) fn invalidate(&self, name: &str, sandbox: &SandboxRef, paths: &Paths) {
        let key = search_key(name, sandbox, paths);
        self.detected.invalidate(&key);
        self.doctor.invalidate(&key);
    }
}

impl Default for BackendSearches {
    fn default() -> Self {
        Self::new()
    }
}

/// Der Schlüssel einer Suche: das Backend, die Teile der Gruppe `sandbox`,
/// die eine Suche liest, und die Umgebung des Daemons.
///
/// Aus der Gruppe fällt heraus, was keine Suche liest: Backend, Untergrenze,
/// Profil, Ordner, Modus und die Umgebung der Sandbox. So teilen sich die
/// Liste für die Einstellung und die mit dem Backend einer Sitzung dieselbe
/// Suche, und keine Variable der Sandbox liegt im Gedächtnis.
pub(crate) fn search_key(name: &str, sandbox: &SandboxRef, paths: &Paths) -> String {
    let read = searched_settings(sandbox);
    format!("{name}\n{read:?}\n{paths:?}")
}

/// Die Teile der Gruppe `sandbox`, die eine Suche oder ein Start eines
/// Backends liest: die Einstellungen der Backends. Profil, Ordner, Modus,
/// Untergrenze, Wahl des Backends und die Umgebung der Sandbox stehen auf
/// ihrem Vorgabewert.
pub(crate) fn searched_settings(sandbox: &SandboxRef) -> SandboxRef {
    let blank = SandboxRef::default();
    let mut read = sandbox.clone();
    read.profile = blank.profile;
    read.work_dir = blank.work_dir;
    read.work_mode = blank.work_mode;
    read.mode = blank.mode;
    read.env = blank.env;
    read.backend = blank.backend;
    read.min_isolation = blank.min_isolation;
    read
}

/// Der Abdruck dieser Dateien als Text, eine Zeile je Datei: Pfad, und wenn
/// es sie gibt, Gerät, Inode, Größe, `mtime` und `ctime` ([`Stamp`]). Gleich
/// genau dann, wenn sich keine der Dateien geändert hat; der Schlüssel des
/// gespeicherten Probestarts im Doctor (HUM-506).
pub(crate) fn fingerprint(files: &[PathBuf]) -> String {
    use std::fmt::Write as _;

    let mut text = String::new();
    for file in files {
        let stamp = Stamp::of(file);
        let _ = write!(text, "{}", stamp.path.display());
        match stamp.meta {
            Some(meta) => {
                for value in meta {
                    let _ = write!(text, " {value}");
                }
            }
            None => text.push_str(" -"),
        }
        text.push('\n');
    }
    text
}

/// Jeder Ort im `PATH` von `env`, an dem ein Programm `name` liegen könnte,
/// ob es dort eines gibt oder nicht: Taucht eines auf, ändert sich der
/// Abdruck.
pub(crate) fn path_candidates(env: &Env, name: &str) -> Vec<PathBuf> {
    let path = env.non_empty("PATH").unwrap_or(DEFAULT_PATH);
    std::env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty())
        .map(|dir| dir.join(name))
        .collect()
}

/// Der Befund eines Backends, dessen Suche nach der Frist der Antwort noch
/// läuft: `DOCTOR_012` als Warnung, denn nachgesehen hat noch niemand.
///
/// Der Vorschlag ist `words`, ein Befehl, der ohne Frist auf das Ergebnis
/// wartet.
pub(crate) fn still_searching(name: &str, running: Duration, words: &[&str]) -> Diagnostic {
    Diagnostic::builder(DOCTOR_012, Severity::Warning)
        .why(pending_reason(name, running))
        .fix(crate::doctor::command_fix(words))
        .build()
}

/// Der Grund einer Suche, die noch läuft, als Satz.
pub(crate) fn pending_reason(name: &str, running: Duration) -> String {
    format!(
        "still being checked: the search for the backend {name} has run for {} ms, longer than \
         this answer waits; it goes on in the daemon, and the next answer has its result",
        running.as_millis()
    )
}

/// Was das Warten auf eine Suche ergab.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Waited<T> {
    /// Das Ergebnis.
    Done(T),
    /// Die Suche lief bei Ablauf der Frist noch, seit so langer Zeit.
    Pending(Duration),
}

/// Eine Suche, die noch nicht läuft: Sie liefert ihr Ergebnis, oder keines,
/// wenn sie ohne Ergebnis endet (dann sucht die nächste Frage neu).
type Job<T> = Box<dyn FnOnce() -> Option<T> + Send>;

/// Ein Gedächtnis für Suchen mit Ergebnissen vom Typ `T`.
///
/// Je Schlüssel läuft höchstens eine Suche. Ändert sich eine Datei, während
/// sie läuft, gilt ihr Ergebnis als veraltet und wird nicht wiederverwendet;
/// die nächste Suche startet erst, wenn die laufende endet. So wächst die
/// Zahl der Threads auch dann nicht, wenn eine Suche hängt, eine Datei sich
/// ständig ändert und die Oberfläche alle zwei Sekunden fragt.
pub(crate) struct Memo<T> {
    slots: Arc<Mutex<HashMap<String, Slot<T>>>>,
    ttl: Duration,
}

impl<T> Clone for Memo<T> {
    fn clone(&self) -> Self {
        Self {
            slots: Arc::clone(&self.slots),
            ttl: self.ttl,
        }
    }
}

impl<T> fmt::Debug for Memo<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Memo")
            .field("slots", &lock(&self.slots).len())
            .field("ttl", &self.ttl)
            .finish()
    }
}

/// Eine Suche im Gedächtnis: der Abdruck ihrer Dateien beim Start, ihr Stand
/// und ob ihr Ergebnis schon vor ihrem Ende veraltet ist.
struct Slot<T> {
    fingerprint: Vec<Stamp>,
    shared: Arc<Shared<T>>,
    stale: bool,
}

impl<T> Slot<T> {
    /// Ob der Eintrag bleibt: solange seine Suche läuft immer, sonst nur,
    /// solange sein Ergebnis gilt.
    fn keep(&self, now: Instant, ttl: Duration) -> bool {
        self.shared.busy() || (!self.stale && !self.shared.expired(now, ttl))
    }
}

/// Der Stand einer Suche, geteilt zwischen dem Thread, der sucht, und allen,
/// die warten.
struct Shared<T> {
    state: Mutex<State<T>>,
    changed: Condvar,
    started: Instant,
    /// Die Suche, bis ihr Thread sie übernimmt.
    job: Mutex<Option<Job<T>>>,
}

enum State<T> {
    /// Sie läuft oder wartet auf ihren Thread.
    Running,
    /// Sie ist fertig, seit `at`.
    Done { at: Instant, value: T },
    /// Sie ist ohne Ergebnis abgebrochen; die nächste Frage sucht neu.
    Lost,
}

impl<T> Shared<T> {
    fn new(started: Instant, job: Job<T>) -> Self {
        Self {
            state: Mutex::new(State::Running),
            changed: Condvar::new(),
            started,
            job: Mutex::new(Some(job)),
        }
    }

    /// Ob ein Thread gerade an dieser Suche arbeitet.
    fn busy(&self) -> bool {
        matches!(&*lock(&self.state), State::Running)
    }

    /// Ob die nächste Frage neu suchen muss.
    fn expired(&self, now: Instant, ttl: Duration) -> bool {
        match &*lock(&self.state) {
            State::Running => false,
            State::Done { at, .. } => now.saturating_duration_since(*at) >= ttl,
            State::Lost => true,
        }
    }
}

impl<T: Clone + Send + 'static> Memo<T> {
    /// Ein leeres Gedächtnis, dessen Ergebnisse `ttl` lang gelten.
    pub(crate) fn new(ttl: Duration) -> Self {
        Self {
            slots: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    /// Die Suche unter `key`: die laufende oder gültige, wenn sich keine der
    /// Dateien `files` seit ihrem Start geändert hat, sonst eine neue mit
    /// `work` in einem eigenen Thread. `work` liefert `None`, wenn die Suche
    /// ohne Ergebnis endet. Läuft unter `key` eine Suche über veraltete
    /// Dateien, startet die neue erst nach ihrem Ende ([`Ticket::wait`]).
    /// Kehrt sofort zurück.
    pub(crate) fn start(
        &self,
        key: String,
        files: &[PathBuf],
        work: impl FnOnce() -> Option<T> + Send + 'static,
    ) -> Ticket<T> {
        self.start_job(key, files, Box::new(work))
    }

    fn start_job(&self, key: String, files: &[PathBuf], work: Job<T>) -> Ticket<T> {
        let fingerprint: Vec<Stamp> = files.iter().map(|file| Stamp::of(file)).collect();
        let now = Instant::now();
        let mut slots = lock(&self.slots);
        // Was abgelaufen ist, fliegt hier hinaus: Das Gedächtnis wächst nicht
        // über die Eingaben hinaus, die gerade gefragt werden. Eine laufende
        // Suche bleibt, bis sie endet.
        slots.retain(|_, slot| slot.keep(now, self.ttl));
        if let Some(slot) = slots.get_mut(&key) {
            let current = slot.fingerprint == fingerprint && !slot.stale;
            if slot.shared.busy() {
                if current {
                    return Ticket::joined(&slot.shared);
                }
                slot.stale = true;
                return Ticket(Kind::After(Box::new(After {
                    previous: Arc::clone(&slot.shared),
                    memo: self.clone(),
                    key,
                    files: files.to_vec(),
                    work: Mutex::new(Some(work)),
                    next: Mutex::new(None),
                })));
            }
            if current && !slot.shared.expired(now, self.ttl) {
                return Ticket::joined(&slot.shared);
            }
        }
        let shared = Arc::new(Shared::new(now, work));
        slots.insert(
            key,
            Slot {
                fingerprint,
                shared: Arc::clone(&shared),
                stale: false,
            },
        );
        drop(slots);
        match spawn(&shared) {
            Ok(()) => Ticket::joined(&shared),
            Err(job) => Ticket(Kind::Direct(Box::new(Direct {
                job: Mutex::new(Some(job)),
                result: Mutex::new(None),
                started: now,
            }))),
        }
    }

    /// Das Ergebnis unter `key` gilt nicht mehr: Die nächste Frage sucht
    /// neu, nach dem Ende einer Suche, die gerade läuft. Für einen
    /// ausdrücklichen Wunsch, etwa den Probestart des Doctors.
    pub(crate) fn invalidate(&self, key: &str) {
        let mut slots = lock(&self.slots);
        if let Some(slot) = slots.get_mut(key) {
            if slot.shared.busy() {
                slot.stale = true;
            } else {
                slots.remove(key);
            }
        }
    }
}

/// Übergibt die Suche einem eigenen Thread.
///
/// Lässt sich keiner starten, endet die Suche im Gedächtnis ohne Ergebnis
/// wie nach einer Panik: Wer auf sie wartet, kehrt zurück, und die nächste
/// Frage sucht neu. Die Suche selbst geht an den Aufrufer zurück, der sie
/// nur ohne Frist und außerhalb des Gedächtnisses selbst ausführt.
fn spawn<T: Send + 'static>(shared: &Arc<Shared<T>>) -> Result<(), Job<T>> {
    let worker = Arc::clone(shared);
    #[cfg(test)]
    let forced = tests::FAIL_SPAWN.with(|hook| hook.borrow_mut().take());
    #[cfg(not(test))]
    let forced: Option<Box<dyn FnOnce()>> = None;
    let spawned = match forced {
        Some(hook) => {
            hook();
            Err(())
        }
        None => std::thread::Builder::new()
            .name("backend-search".to_owned())
            .spawn(move || {
                let job = lock(&worker.job).take();
                if let Some(job) = job {
                    finish(&worker, job);
                }
            })
            .map(|_| ())
            .map_err(|_| ()),
    };
    if spawned.is_ok() {
        return Ok(());
    }
    let job = lock(&shared.job).take();
    *lock(&shared.state) = State::Lost;
    shared.changed.notify_all();
    // Ohne Thread hat niemand die Suche übernommen; sie ist noch da.
    job.map_or(Ok(()), Err)
}

/// Führt `job` aus und legt das Ergebnis ab. Endet sie ohne Ergebnis oder mit
/// einer Panik, bleibt kein Wartender hängen: Der Stand wird [`State::Lost`].
fn finish<T>(shared: &Shared<T>, job: Job<T>) {
    let state = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
        Ok(Some(value)) => State::Done {
            at: Instant::now(),
            value,
        },
        Ok(None) | Err(_) => State::Lost,
    };
    *lock(&shared.state) = state;
    shared.changed.notify_all();
}

/// Der Anspruch auf das Ergebnis einer Suche.
pub(crate) struct Ticket<T>(Kind<T>);

enum Kind<T> {
    /// Die Suche läuft oder ist fertig.
    Joined(Arc<Shared<T>>),
    /// Es ließ sich kein Thread starten: Die Suche läuft nur für einen
    /// Aufrufer ohne Frist, auf seinem Thread, außerhalb des Gedächtnisses.
    Direct(Box<Direct<T>>),
    /// Die Suche startet erst nach dem Ende einer veralteten.
    After(Box<After<T>>),
}

/// Eine Suche ohne eigenen Thread, die nur ein Aufrufer ohne Frist ausführt.
struct Direct<T> {
    job: Mutex<Option<Job<T>>>,
    result: Mutex<Option<Waited<T>>>,
    started: Instant,
}

impl<T: Clone> Direct<T> {
    fn wait(&self, deadline: Option<Instant>) -> Waited<T> {
        let mut result = lock(&self.result);
        if let Some(done) = result.as_ref() {
            return done.clone();
        }
        let job = if deadline.is_none() {
            lock(&self.job).take()
        } else {
            None
        };
        let Some(job) = job else {
            return Waited::Pending(self.started.elapsed());
        };
        let done = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(job)) {
            Ok(Some(value)) => Waited::Done(value),
            Ok(None) | Err(_) => Waited::Pending(self.started.elapsed()),
        };
        *result = Some(done.clone());
        done
    }
}

/// Eine Suche, die auf das Ende der veralteten unter demselben Schlüssel
/// wartet, bevor sie startet.
struct After<T> {
    previous: Arc<Shared<T>>,
    memo: Memo<T>,
    key: String,
    files: Vec<PathBuf>,
    work: Mutex<Option<Job<T>>>,
    next: Mutex<Option<Ticket<T>>>,
}

impl<T: Clone + Send + 'static> Ticket<T> {
    fn joined(shared: &Arc<Shared<T>>) -> Self {
        Self(Kind::Joined(Arc::clone(shared)))
    }

    /// Wartet bis `deadline` auf das Ergebnis, ohne Frist bis zum Ende der
    /// Suche.
    pub(crate) fn wait(&self, deadline: Option<Instant>) -> Waited<T> {
        match &self.0 {
            Kind::Joined(shared) => wait_on(shared, deadline),
            Kind::Direct(direct) => direct.wait(deadline),
            Kind::After(after) => {
                if let Some(running) = settle(&after.previous, deadline) {
                    return Waited::Pending(running);
                }
                let mut next = lock(&after.next);
                if next.is_none() {
                    let work = lock(&after.work).take();
                    if let Some(work) = work {
                        *next = Some(after.memo.start_job(after.key.clone(), &after.files, work));
                    }
                }
                match next.as_ref() {
                    Some(ticket) => ticket.wait(deadline),
                    None => Waited::Pending(after.previous.started.elapsed()),
                }
            }
        }
    }
}

/// Wartet, bis die Suche nicht mehr läuft, höchstens bis `deadline`; sonst
/// wie lange sie schon läuft.
fn settle<T>(shared: &Shared<T>, deadline: Option<Instant>) -> Option<Duration> {
    match wait_state(shared, deadline, |_| ()) {
        Waited::Pending(running) => Some(running),
        Waited::Done(()) => None,
    }
}

/// Wartet bis `deadline` auf das Ergebnis der Suche.
fn wait_on<T: Clone>(shared: &Shared<T>, deadline: Option<Instant>) -> Waited<T> {
    match wait_state(shared, deadline, |state| match state {
        State::Done { value, .. } => Some(value.clone()),
        _ => None,
    }) {
        Waited::Done(Some(value)) => Waited::Done(value),
        Waited::Done(None) => Waited::Pending(shared.started.elapsed()),
        Waited::Pending(running) => Waited::Pending(running),
    }
}

/// Wartet bis `deadline`, bis die Suche endet, und liest dann `read` aus
/// ihrem Stand.
fn wait_state<T, R>(
    shared: &Shared<T>,
    deadline: Option<Instant>,
    read: impl Fn(&State<T>) -> R,
) -> Waited<R> {
    let mut state = lock(&shared.state);
    loop {
        if !matches!(&*state, State::Running) {
            return Waited::Done(read(&state));
        }
        state = match deadline {
            None => shared
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner),
            Some(deadline) => {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() {
                    return Waited::Pending(shared.started.elapsed());
                }
                shared
                    .changed
                    .wait_timeout(state, left)
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
            }
        };
    }
}

/// Der Abdruck einer Datei: Pfad, und wenn es sie gibt, Gerät, Inode,
/// Größe, `mtime` und `ctime` je in Nanosekunden. `ctime` fängt auch eine
/// geänderte Berechtigung (etwa von `/dev/kvm`).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Stamp {
    path: PathBuf,
    meta: Option<[i128; 5]>,
}

impl Stamp {
    fn of(path: &Path) -> Self {
        use std::os::unix::fs::MetadataExt as _;

        let meta = std::fs::metadata(path).ok().map(|meta| {
            [
                i128::from(meta.dev()),
                i128::from(meta.ino()),
                i128::from(meta.size()),
                i128::from(meta.mtime()) * 1_000_000_000 + i128::from(meta.mtime_nsec()),
                i128::from(meta.ctime()) * 1_000_000_000 + i128::from(meta.ctime_nsec()),
            ]
        });
        Self {
            path: path.to_path_buf(),
            meta,
        }
    }
}

/// Sperrt, auch nach einer Panik in einem anderen Thread: Der Stand hinter
/// der Sperre ist immer ganz geschrieben.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::*;

    thread_local! {
        /// Lässt den nächsten Start eines Threads auf diesem Thread
        /// scheitern, nachdem es dies ausgeführt hat.
        pub(super) static FAIL_SPAWN: RefCell<Option<Box<dyn FnOnce()>>> =
            const { RefCell::new(None) };
    }

    fn counted(
        calls: &Arc<AtomicUsize>,
        value: u32,
    ) -> impl FnOnce() -> Option<u32> + Send + 'static {
        let calls = Arc::clone(calls);
        move || {
            calls.fetch_add(1, Ordering::SeqCst);
            Some(value)
        }
    }

    /// Eine Suche, die 10 s braucht, hält eine Frist von 300 ms nicht auf:
    /// Sie steht als laufend da, und eine schnelle daneben ist fertig.
    /// Mutationsbeweis: ohne eigenen Thread (Suche im Aufrufer) braucht der
    /// Test 10 s und scheitert an der Zeit.
    #[test]
    fn a_slow_search_is_pending_at_the_deadline_and_a_fast_one_is_done() {
        let memo = Memo::<u32>::new(SEARCH_TTL);
        let started = Instant::now();
        let slow = memo.start("slow".to_owned(), &[], || {
            std::thread::sleep(Duration::from_secs(10));
            Some(1)
        });
        let fast = memo.start("fast".to_owned(), &[], || Some(2));
        let deadline = Some(started + Duration::from_millis(300));
        assert_eq!(fast.wait(deadline), Waited::Done(2));
        assert!(matches!(slow.wait(deadline), Waited::Pending(_)));
        assert!(
            started.elapsed() < Duration::from_secs(2),
            "{:?}",
            started.elapsed()
        );
    }

    /// Ohne Frist wartet der Aufrufer auf das Ergebnis.
    #[test]
    fn without_a_deadline_the_caller_waits_for_the_result() {
        let memo = Memo::<u32>::new(SEARCH_TTL);
        let ticket = memo.start("k".to_owned(), &[], || {
            std::thread::sleep(Duration::from_millis(200));
            Some(7)
        });
        assert_eq!(ticket.wait(None), Waited::Done(7));
    }

    /// Ein gültiges Ergebnis kommt ohne neue Suche, und wer während einer
    /// laufenden fragt, wartet auf dieselbe. Ändert sich eine der Dateien,
    /// sucht die nächste Frage neu. Mutationsbeweis: ohne den Vergleich des
    /// Abdrucks bleibt die dritte Suche aus.
    #[test]
    fn the_result_is_kept_until_a_file_changes() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("program");
        std::fs::write(&program, "one").unwrap();
        let files = vec![program.clone(), dir.path().join("absent")];
        let memo = Memo::<u32>::new(SEARCH_TTL);
        let calls = Arc::new(AtomicUsize::new(0));

        assert_eq!(
            memo.start("k".to_owned(), &files, counted(&calls, 1))
                .wait(None),
            Waited::Done(1)
        );
        assert_eq!(
            memo.start("k".to_owned(), &files, counted(&calls, 2))
                .wait(None),
            Waited::Done(1)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1, "kept, not searched again");

        std::fs::write(&program, "a longer body").unwrap();
        assert_eq!(
            memo.start("k".to_owned(), &files, counted(&calls, 3))
                .wait(None),
            Waited::Done(3)
        );
        // Ein Programm, das im `PATH` neu auftaucht, zählt genauso.
        std::fs::write(dir.path().join("absent"), "").unwrap();
        assert_eq!(
            memo.start("k".to_owned(), &files, counted(&calls, 4))
                .wait(None),
            Waited::Done(4)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    /// Nach der Lebensdauer sucht die nächste Frage neu; ein anderer
    /// Schlüssel hat seine eigene Suche.
    #[test]
    fn an_expired_result_and_another_key_search_again() {
        let memo = Memo::<u32>::new(Duration::from_millis(50));
        let calls = Arc::new(AtomicUsize::new(0));
        memo.start("k".to_owned(), &[], counted(&calls, 1))
            .wait(None);
        memo.start("other".to_owned(), &[], counted(&calls, 2))
            .wait(None);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(
            memo.start("k".to_owned(), &[], counted(&calls, 3))
                .wait(None),
            Waited::Done(3)
        );
    }

    /// Wer fragt, während die Suche läuft, startet keine zweite.
    #[test]
    fn a_running_search_is_joined_not_repeated() {
        let memo = Memo::<u32>::new(SEARCH_TTL);
        let calls = Arc::new(AtomicUsize::new(0));
        let first = {
            let calls = Arc::clone(&calls);
            memo.start("k".to_owned(), &[], move || {
                calls.fetch_add(1, Ordering::SeqCst);
                std::thread::sleep(Duration::from_millis(200));
                Some(1)
            })
        };
        let second = memo.start("k".to_owned(), &[], counted(&calls, 2));
        assert_eq!(second.wait(None), Waited::Done(1));
        assert_eq!(first.wait(None), Waited::Done(1));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    /// Eine Suche, die mit einer Panik abbricht, lässt niemanden hängen, und
    /// die nächste Frage sucht neu.
    #[test]
    fn a_search_that_panics_is_not_waited_for_forever() {
        let memo = Memo::<u32>::new(SEARCH_TTL);
        let lost = memo.start("k".to_owned(), &[], || panic!("search broke"));
        assert!(matches!(lost.wait(None), Waited::Pending(_)));
        let calls = Arc::new(AtomicUsize::new(0));
        assert_eq!(
            memo.start("k".to_owned(), &[], counted(&calls, 5))
                .wait(None),
            Waited::Done(5)
        );
    }

    /// Eine Suche, die hängt, während sich ihre Datei bei jeder Frage
    /// ändert: Je Schlüssel läuft trotzdem nur eine Suche, ihr Ergebnis wird
    /// nicht wiederverwendet, und die nächste startet erst nach ihrem Ende.
    /// Mutationsbeweis: Ersetzt eine geänderte Datei die laufende Suche, statt
    /// auf sie zu warten, starten hier sechs.
    #[test]
    fn a_hung_search_and_a_changing_file_keep_one_search_per_key() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("program");
        let files = vec![program.clone()];
        let memo = Memo::<u32>::new(SEARCH_TTL);
        let started = Arc::new(AtomicUsize::new(0));
        let release = Arc::new(AtomicBool::new(false));
        let hung = |value: u32| {
            let (started, release) = (Arc::clone(&started), Arc::clone(&release));
            move || {
                started.fetch_add(1, Ordering::SeqCst);
                while !release.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                Some(value)
            }
        };
        std::fs::write(&program, "0").unwrap();
        let first = memo.start("k".to_owned(), &files, hung(0));
        let mut later = Vec::new();
        for round in 1..=5u32 {
            std::fs::write(&program, "x".repeat(round as usize + 1)).unwrap();
            let ticket = memo.start("k".to_owned(), &files, hung(round));
            let deadline = Instant::now() + Duration::from_millis(20);
            assert!(matches!(ticket.wait(Some(deadline)), Waited::Pending(_)));
            later.push(ticket);
        }
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(started.load(Ordering::SeqCst), 1, "one search at a time");

        release.store(true, Ordering::SeqCst);
        assert_eq!(first.wait(None), Waited::Done(0));
        // Nach dem Ende der veralteten startet genau eine neue, und alle
        // späteren Fragen teilen sie.
        let results: Vec<Waited<u32>> = later.iter().map(|t| t.wait(None)).collect();
        assert!(results.iter().all(|r| *r == results[0]), "{results:?}");
        assert_ne!(
            results[0],
            Waited::Done(0),
            "the stale result is not reused"
        );
        assert_eq!(started.load(Ordering::SeqCst), 2);
    }

    /// Lässt sich kein Thread starten, endet die Suche im Gedächtnis ohne
    /// Ergebnis: Wer schon auf sie wartet, kehrt zurück, und es startet keine
    /// zweite. Der Aufrufer ohne Frist sucht selbst, einer mit Frist nie.
    /// Mutationsbeweis: ohne `Lost` und `notify_all` wartet der zweite
    /// Aufrufer für immer, und der Test scheitert an der Frist von 2 s.
    #[test]
    fn a_failed_thread_start_leaves_nobody_waiting_and_runs_only_without_a_deadline() {
        let memo = Memo::<u32>::new(SEARCH_TTL);
        let calls = Arc::new(AtomicUsize::new(0));
        let joined: Arc<Mutex<Option<Ticket<u32>>>> = Arc::new(Mutex::new(None));
        {
            let (memo, calls, joined) = (memo.clone(), Arc::clone(&calls), Arc::clone(&joined));
            FAIL_SPAWN.with(|hook| {
                *hook.borrow_mut() = Some(Box::new(move || {
                    // Ein zweiter Aufrufer kommt, während die Suche noch als
                    // laufend gilt.
                    *lock(&joined) = Some(memo.start("k".to_owned(), &[], counted(&calls, 2)));
                }));
            });
        }
        let direct = memo.start("k".to_owned(), &[], counted(&calls, 1));
        let second = lock(&joined).take().expect("the second caller came");
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(second.wait(None));
        });
        assert!(matches!(
            receiver.recv_timeout(Duration::from_secs(2)),
            Ok(Waited::Pending(_))
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0, "no second search");

        let deadline = Instant::now() + Duration::from_millis(20);
        assert!(matches!(direct.wait(Some(deadline)), Waited::Pending(_)));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(direct.wait(None), Waited::Done(1));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        // Das Gedächtnis hält nichts davon: Die nächste Frage sucht neu.
        assert_eq!(
            memo.start("k".to_owned(), &[], counted(&calls, 3))
                .wait(None),
            Waited::Done(3)
        );
    }

    /// Eine Suche ohne Ergebnis gilt als abgebrochen, nicht als Ergebnis.
    #[test]
    fn a_search_without_a_result_is_searched_again() {
        let memo = Memo::<u32>::new(SEARCH_TTL);
        assert!(matches!(
            memo.start("k".to_owned(), &[], || None).wait(None),
            Waited::Pending(_)
        ));
        let calls = Arc::new(AtomicUsize::new(0));
        assert_eq!(
            memo.start("k".to_owned(), &[], counted(&calls, 3))
                .wait(None),
            Waited::Done(3)
        );
    }

    /// Ein ausdrücklicher Wunsch verwirft ein gültiges Ergebnis.
    #[test]
    fn an_invalidated_result_is_searched_again() {
        let memo = Memo::<u32>::new(SEARCH_TTL);
        let calls = Arc::new(AtomicUsize::new(0));
        memo.start("k".to_owned(), &[], counted(&calls, 1))
            .wait(None);
        memo.invalidate("k");
        assert_eq!(
            memo.start("k".to_owned(), &[], counted(&calls, 2))
                .wait(None),
            Waited::Done(2)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    /// Der Schlüssel hängt nicht an dem, was keine Suche liest: Backend und
    /// Untergrenze der Gruppe, und keine Variable der Sandbox steht darin.
    #[test]
    fn the_key_ignores_what_no_search_reads() {
        let paths = Paths::new(Env::from_pairs([("PATH", "/bin")]));
        let plain = SandboxRef::default();
        let mut other = SandboxRef {
            min_isolation: humanitl_config::MinIsolation::Kernel,
            ..SandboxRef::default()
        };
        other
            .env
            .insert("TOKEN".to_owned(), "secret-value".to_owned());
        assert_eq!(
            search_key("x", &plain, &paths),
            search_key("x", &other, &paths)
        );
        assert!(!search_key("x", &other, &paths).contains("secret-value"));
        assert_ne!(
            search_key("x", &plain, &paths),
            search_key("y", &plain, &paths)
        );
    }
}
