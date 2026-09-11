//! Der Schreiber des Audit-Logs (HUM-050).
//!
//! Genau ein Thread schreibt, und alle anderen schicken ihm Records über einen
//! Kanal: Es gibt keinen parallelen Zugriff auf die Datei, und `seq` und der
//! Hash des Endes leben nur dort. Die Spezifikation nennt dafür einen
//! `tokio::sync::mpsc`-Consumer; hier ist es ein `std::sync::mpsc`-Kanal vor
//! einem eigenen Thread, genau wie beim Schreiber der Aufzeichnung. Der Grund
//! ist die fsync-Politik: Sie braucht ein Warten mit Frist
//! (`recv_timeout`), und das Schreiben selbst blockiert ohnehin.
//!
//! **Beim Öffnen**, in dieser Reihenfolge:
//!
//! 1. Verzeichnis `0700`, Datei `0600`, `O_APPEND`, kein Symlink.
//! 2. Eine exklusive Sperre (`flock`) auf die Datei, ohne zu warten. Hält sie
//!    ein anderer Daemon, endet der Start mit [`AUDIT_004`].
//! 3. Endet die Datei nicht mit `\n`, ist der Rest hinter dem letzten
//!    Zeilenumbruch eine abgerissene Zeile: Er wird nach
//!    `audit.jsonl.corrupt-<ts>` verschoben, die Datei gekürzt, und
//!    [`AUDIT_002`] meldet es. Die Kette läuft am letzten vollständigen Record
//!    weiter, ohne Lücke.
//! 4. Der letzte vollständige Record muss kanonisch sein und zu Hash und
//!    Schlüssel passen, und ein Anker unter seiner Nummer muss seinen Hash
//!    nennen. Sonst [`AUDIT_001`], und der Daemon startet nicht: Eine Kette auf
//!    einem gebrochenen Ende fortzusetzen hieße, jeden neuen Record auf etwas
//!    zu bauen, das die Prüfung ohnehin verwirft. Der `fix` legt die Datei
//!    beiseite, und der nächste Start ist Schritt 5.
//! 5. Liegt ein Anker hinter dem Ende, wurde das Log gekürzt, ersetzt oder
//!    beiseitegelegt. Der Daemon startet trotzdem, mit [`AUDIT_007`]: Die
//!    Kette läuft hinter dem letzten Anker weiter, und ihr erster Record ist
//!    ein `audit.resumed`, dessen Nummer auf den Anker folgt und dessen `prev`
//!    der Hash des Ankers ist. So bekommt kein neuer Record die Nummer eines
//!    alten Ankers, die Anker bleiben als Beleg in `audit_anchors`, und
//!    `verify` meldet die Lücke weiter als Bruch. Ein Startverbot schützte hier
//!    keinen Beleg, der nicht schon gesichert ist, und sperrte den Nutzer aus
//!    der Sandbox aus, bis jemand die Tabelle von Hand leert.
//!
//! **Anker.** Ist die nächste Nummer ein Vielfaches von `anchor_every`, folgt
//! dem gerade geschriebenen Record sofort ein `audit.anchor` unter dieser
//! Nummer. Er nennt in `data` seinen Vorgänger, und seine eigene Nummer samt
//! Hash geht als [`Anchor`] in die zweite Ablage (`audit_anchors` in `SQLite`);
//! über `prev` deckt sein Hash die ganze Kette davor. Beim Beenden folgt
//! derselbe Anker hinter `daemon.stopped`, sofern nicht gerade einer
//! geschrieben wurde. So endet ein geordnet beendetes Log immer auf einem
//! verankerten Record, und bei `anchor_every = 3` stehen nach sieben Records
//! die Anker bei 3 und 6. Bevor ein Anker in die Datenbank geht, ist die Datei
//! auf der Platte: Ein Anker auf eine Zeile, die ein Absturz noch verlieren
//! kann, wäre ein falscher Befund „gekürzt".
//!
//! **fsync.** Nach `fsync_every` Records oder spätestens nach
//! `fsync_interval`, außerdem vor jedem Anker und beim Beenden.

use std::fmt;
use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::{self, Write as _};
use std::os::unix::fs::{
    DirBuilderExt as _, FileExt as _, OpenOptionsExt as _, PermissionsExt as _,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use chrono::Utc;
use humanitl_core::diagnostics::codes::{
    AUDIT_001, AUDIT_002, AUDIT_003, AUDIT_004, AUDIT_006, AUDIT_007,
};
use humanitl_core::{Diagnostic, FixAction, SessionId, Severity};
use serde_json::{Value, json};
use tokio::sync::broadcast;
use zeroize::Zeroizing;

use crate::Anchor;
use crate::canonical::canonical_json;
use crate::key::{AuditKey, KEY_LEN, shell_quote};
use crate::kinds::{AnchorData, AuditResumed, DaemonStopped, RecordKind};
use crate::record::{AuditRecord, GENESIS_PREV, NO_SESSION, RecordBody, format_ts, mac_matches};
use crate::verify::set_aside_fix;

/// Vorgabe für `audit.anchor_every`.
pub const DEFAULT_ANCHOR_EVERY: u32 = 100;
/// Vorgabe für `audit.fsync_every`.
pub const DEFAULT_FSYNC_EVERY: u32 = 50;
/// Spätestens nach dieser Zeit ist ein geschriebener Record auf der Platte.
pub const FSYNC_INTERVAL: Duration = Duration::from_secs(1);
/// Rechte von `audit.jsonl`.
pub const LOG_MODE: u32 = 0o600;
/// Rechte des Audit-Verzeichnisses.
pub const LOG_DIR_MODE: u32 = 0o700;
/// So lang wird keine Zeile; eine längere am Ende der Datei ist kein Record.
pub const MAX_LINE_BYTES: u64 = 1024 * 1024;

/// So viele Befunde warten höchstens im Strom.
const DIAGNOSTIC_BUFFER: usize = 64;
/// In diesen Stücken wird die Datei von hinten gelesen.
const TAIL_CHUNK: u64 = 64 * 1024;

/// Wann geankert und synchronisiert wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WriterOptions {
    /// Jeder Record, dessen Nummer ein Vielfaches davon ist, ist ein Anker.
    /// `0` heißt: nur beim Beenden.
    pub anchor_every: u32,
    /// Nach so vielen Records wird synchronisiert; `0` gilt als `1`.
    pub fsync_every: u32,
    /// Spätestens nach dieser Zeit wird synchronisiert.
    pub fsync_interval: Duration,
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self {
            anchor_every: DEFAULT_ANCHOR_EVERY,
            fsync_every: DEFAULT_FSYNC_EVERY,
            fsync_interval: FSYNC_INTERVAL,
        }
    }
}

/// Die zweite Ablage der Anker: im Daemon die Tabelle `audit_anchors`.
///
/// Ein Rückruf und kein Port: Diese Crate darf die Aufzeichnung nicht kennen
/// (`backlog/CONVENTIONS.md` 3.1), und der Daemon verdrahtet beide.
pub type AnchorMirror = Box<dyn FnMut(&Anchor) -> Result<(), Diagnostic> + Send>;

/// Das Ende der Kette: letzte Nummer und ihr Hash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Head {
    /// Die letzte Nummer; `0` für eine leere Kette.
    pub seq: u64,
    /// Ihr Hash; für eine leere Kette [`GENESIS_PREV`].
    pub hash: String,
}

enum Command {
    Record {
        session: Option<SessionId>,
        kind: RecordKind,
    },
    Sync(mpsc::Sender<Head>),
    Stop {
        reason: String,
        reply: mpsc::Sender<Head>,
    },
    Close,
}

/// Das Handle, über das jeder Teil des Daemons Records schickt.
///
/// Billig zu klonen. `record` blockiert nie; ist der Schreiber schon beendet,
/// geht der Record verloren, und `tracing` sagt es.
#[derive(Clone)]
pub struct AuditHandle {
    tx: mpsc::Sender<Command>,
}

impl fmt::Debug for AuditHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditHandle").finish_non_exhaustive()
    }
}

impl AuditHandle {
    /// Schickt einen Record. `None` als Sitzung steht im Log als `-`.
    ///
    /// Einen `audit.anchor` nimmt das Handle nicht an: Anker schreibt nur der
    /// Schreiber, weil nur er ihn auch in die zweite Ablage legt.
    pub fn record(&self, session: Option<SessionId>, kind: RecordKind) {
        if matches!(kind, RecordKind::AuditAnchor(_)) {
            tracing::warn!("an audit.anchor from outside the writer was refused");
            return;
        }
        if self.tx.send(Command::Record { session, kind }).is_err() {
            tracing::warn!("the audit writer has ended; a record was dropped");
        }
    }

    /// Wartet, bis alles bisher Geschickte auf der Platte ist, und nennt das
    /// Ende der Kette. Blockiert; `None`, wenn der Schreiber schon beendet ist.
    #[must_use]
    pub fn sync(&self) -> Option<Head> {
        let (reply, answer) = mpsc::channel();
        self.tx.send(Command::Sync(reply)).ok()?;
        answer.recv().ok()
    }
}

/// Der Schreiber: öffnet die Datei, hält den Thread und beendet ihn.
pub struct AuditWriter {
    handle: AuditHandle,
    join: Option<JoinHandle<()>>,
    diagnostics: broadcast::Sender<Diagnostic>,
    path: PathBuf,
    resumed: Head,
}

impl fmt::Debug for AuditWriter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuditWriter")
            .field("path", &self.path)
            .field("resumed", &self.resumed)
            .finish_non_exhaustive()
    }
}

impl AuditWriter {
    /// Öffnet `path` und startet den Schreib-Thread.
    ///
    /// `anchors` sind die Anker aus der zweiten Ablage, gegen die das Ende der
    /// Datei geprüft wird; `mirror` bekommt jeden neuen Anker. Zurück kommen
    /// der Schreiber und die Befunde des Öffnens, die den Start nicht
    /// aufhalten ([`AUDIT_002`], [`AUDIT_007`]).
    ///
    /// # Errors
    ///
    /// [`AUDIT_006`], wenn Verzeichnis oder Datei nicht benutzbar sind,
    /// [`AUDIT_004`], wenn ein anderer Prozess die Sperre hält, [`AUDIT_001`],
    /// wenn der letzte Record nicht besteht oder ein Anker unter seiner Nummer
    /// einen anderen Hash nennt.
    pub fn open(
        path: &Path,
        key: &AuditKey,
        options: WriterOptions,
        anchors: &[Anchor],
        mirror: Option<AnchorMirror>,
    ) -> Result<(Self, Vec<Diagnostic>), Diagnostic> {
        let mut notes = Vec::new();
        ensure_log_dir(path)?;
        let file = open_log(path)?;
        lock(&file, path)?;
        let mut len = file
            .metadata()
            .map_err(|err| not_writable(path, &format!("cannot read its size: {err}")))?
            .len();
        if len > 0 && byte_at(&file, len - 1).map_err(|err| unreadable(path, &err))? != b'\n' {
            let (complete, note) = set_aside_torn_tail(&file, path, len)?;
            len = complete;
            notes.push(note);
        }
        let head = if len == 0 {
            Head {
                seq: 0,
                hash: GENESIS_PREV.to_owned(),
            }
        } else {
            last_record(&file, path, len, key.bytes())?
        };
        let gap = check_anchors(path, &head, anchors)?;
        let start = gap.as_ref().map_or_else(
            || head.clone(),
            |gap| Head {
                seq: gap.anchor_seq,
                hash: gap.anchor_hash.clone(),
            },
        );

        let (diagnostics, _idle) = broadcast::channel(DIAGNOSTIC_BUFFER);
        let (tx, rx) = mpsc::channel();
        let mut chain = Chain {
            file,
            len,
            key: Zeroizing::new(*key.bytes()),
            last_seq: start.seq,
            last_hash: start.hash.clone(),
            options,
            mirror,
            unsynced: 0,
            last_sync: Instant::now(),
            diagnostics: diagnostics.clone(),
            path: path.to_owned(),
            broken: false,
            last_was_anchor: false,
        };
        if let Some(gap) = gap {
            // Der erste Record hinter der Lücke sagt, dass sie da ist, noch
            // bevor irgendein anderer Teil des Daemons schreibt.
            notes.push(resumed_note(path, &gap));
            chain.record(
                None,
                &RecordKind::AuditResumed(AuditResumed {
                    log_seq: gap.log_seq,
                    anchor_seq: gap.anchor_seq,
                }),
            );
        }
        let join = std::thread::Builder::new()
            .name("humanitl-audit".to_owned())
            .spawn(move || chain.run(&rx))
            .map_err(|err| not_writable(path, &format!("cannot start the writer thread: {err}")))?;
        Ok((
            Self {
                handle: AuditHandle { tx },
                join: Some(join),
                diagnostics,
                path: path.to_owned(),
                resumed: start,
            },
            notes,
        ))
    }

    /// Ein Handle zum Schicken.
    #[must_use]
    pub fn handle(&self) -> AuditHandle {
        self.handle.clone()
    }

    /// Der Strom der Befunde des Schreib-Threads.
    #[must_use]
    pub fn diagnostics(&self) -> broadcast::Receiver<Diagnostic> {
        self.diagnostics.subscribe()
    }

    /// Die Datei.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Das Ende der Kette, an das dieser Schreiber beim Öffnen anhängte.
    #[must_use]
    pub const fn resumed(&self) -> &Head {
        &self.resumed
    }

    /// Schreibt `daemon.stopped` mit `reason`, danach einen Anker, bringt alles
    /// auf die Platte und beendet den Thread. Blockiert.
    ///
    /// Zurück kommt das Ende der Kette; `None`, wenn der Thread schon vorher
    /// endete.
    #[must_use]
    pub fn stop(mut self, reason: &str) -> Option<Head> {
        let (reply, answer) = mpsc::channel();
        let sent = self
            .handle
            .tx
            .send(Command::Stop {
                reason: reason.to_owned(),
                reply,
            })
            .is_ok();
        let head = if sent { answer.recv().ok() } else { None };
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
        head
    }
}

impl Drop for AuditWriter {
    /// Ohne [`AuditWriter::stop`]: alles auf die Platte, kein `daemon.stopped`,
    /// kein Anker. Das ist der Weg eines Abbruchs, nicht der eines Endes.
    fn drop(&mut self) {
        if let Some(join) = self.join.take() {
            let _ = self.handle.tx.send(Command::Close);
            let _ = join.join();
        }
    }
}

/// Die Daten einer Art, oder ein Platzhalter samt Befund, wenn sie sich nicht
/// kanonisch schreiben lassen.
///
/// Der Platzhalter hält die Kette lückenlos: Ein Record, der fehlt, wäre die
/// schlechtere Wahl als einer, der sagt, dass er kaputt ist.
#[must_use]
pub fn checked_data(kind: &str, data: Value) -> (Value, Option<Diagnostic>) {
    if canonical_json(&data).is_ok() {
        return (data, None);
    }
    let diagnostic = Diagnostic::builder(AUDIT_003, Severity::Error)
        .why(format!(
            "the data of a {kind} record holds a number that is not an integer; the record was \
             written with a placeholder instead"
        ))
        .build();
    (json!({ "error": "non_canonical" }), Some(diagnostic))
}

/// Der Zustand des Schreib-Threads.
struct Chain {
    file: File,
    len: u64,
    key: Zeroizing<[u8; KEY_LEN]>,
    last_seq: u64,
    last_hash: String,
    options: WriterOptions,
    mirror: Option<AnchorMirror>,
    unsynced: u32,
    last_sync: Instant,
    diagnostics: broadcast::Sender<Diagnostic>,
    path: PathBuf,
    /// Eine Zeile ließ sich weder schreiben noch zurücknehmen. Danach wird
    /// nichts mehr angehängt: Jede weitere Zeile hinge hinter einem Rest.
    broken: bool,
    /// Der letzte Record ist ein Anker; beim Beenden braucht es keinen zweiten.
    last_was_anchor: bool,
}

impl Chain {
    fn run(mut self, rx: &mpsc::Receiver<Command>) {
        loop {
            let command = if self.unsynced == 0 {
                match rx.recv() {
                    Ok(command) => command,
                    Err(_) => break,
                }
            } else {
                let wait = self
                    .options
                    .fsync_interval
                    .saturating_sub(self.last_sync.elapsed());
                match rx.recv_timeout(wait) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => {
                        self.sync();
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            };
            match command {
                Command::Record { session, kind } => self.record(session, &kind),
                Command::Sync(reply) => {
                    self.sync();
                    let _ = reply.send(self.head());
                }
                Command::Stop { reason, reply } => {
                    self.record(None, &RecordKind::DaemonStopped(DaemonStopped { reason }));
                    if !self.last_was_anchor {
                        self.anchor();
                    }
                    self.sync();
                    let _ = reply.send(self.head());
                    return;
                }
                Command::Close => break,
            }
        }
        self.sync();
    }

    fn head(&self) -> Head {
        Head {
            seq: self.last_seq,
            hash: self.last_hash.clone(),
        }
    }

    fn anchor_due(&self) -> bool {
        let every = u64::from(self.options.anchor_every);
        every > 0 && (self.last_seq + 1).is_multiple_of(every)
    }

    fn record(&mut self, session: Option<SessionId>, kind: &RecordKind) {
        let session = session.map_or_else(|| NO_SESSION.to_owned(), |id| id.to_string());
        let (data, problem) = checked_data(kind.name(), kind.data());
        // Jede Art trägt nur Ganzzahlen; ein Float ist ein Programmfehler, und
        // der soll im Test auffallen, nicht im Feld.
        debug_assert!(
            problem.is_none(),
            "audit data must be canonical: {problem:?}"
        );
        if let Some(problem) = problem {
            self.report(problem);
        }
        if self.append(session, kind.name(), data).is_some() && self.anchor_due() {
            // Sofort und nicht erst vor dem nächsten Record: Ein Anker, der
            // auf den nächsten Vorgang wartet, fehlt genau dann, wenn danach
            // nichts mehr kommt.
            self.anchor();
        }
    }

    /// Schreibt den Anker: einen `audit.anchor`, der seinen Vorgänger nennt,
    /// und seine eigene Nummer samt Hash in die zweite Ablage.
    fn anchor(&mut self) {
        let data = RecordKind::AuditAnchor(AnchorData {
            anchored_seq: self.last_seq,
            anchored_hash: self.last_hash.clone(),
        })
        .data();
        let Some(record) = self.append(NO_SESSION.to_owned(), "audit.anchor", data) else {
            return;
        };
        // Erst auf die Platte, dann in die Datenbank.
        self.sync();
        let anchor = Anchor {
            seq: record.body.seq,
            hash: record.hash,
            ts: record.body.ts,
        };
        if let Some(mirror) = self.mirror.as_mut()
            && let Err(diagnostic) = mirror(&anchor)
        {
            self.report(diagnostic);
        }
    }

    fn append(&mut self, session: String, kind: &str, data: Value) -> Option<AuditRecord> {
        if self.broken {
            return None;
        }
        let body = RecordBody {
            seq: self.last_seq + 1,
            ts: format_ts(Utc::now()),
            session,
            kind: kind.to_owned(),
            data,
            prev: self.last_hash.clone(),
        };
        let sealed = body.seal(&self.key).and_then(|record| {
            let line = record.to_line()?;
            Ok((record, line))
        });
        let Ok((record, mut line)) = sealed else {
            // `checked_data` hat die Daten schon geprüft; das hier ist nur der
            // Riegel dahinter.
            let (_, problem) = checked_data(kind, Value::Null);
            if let Some(problem) = problem {
                self.report(problem);
            }
            return None;
        };
        line.push(b'\n');
        if let Err(err) = self.file.write_all(&line) {
            // Eine halbe Zeile wieder abschneiden; gelingt das nicht, wird
            // nichts mehr angehängt.
            if self.file.set_len(self.len).is_err() {
                self.broken = true;
            }
            self.report(not_writable(
                &self.path,
                &format!("cannot append record {}: {err}", record.body.seq),
            ));
            return None;
        }
        self.len += u64::try_from(line.len()).unwrap_or(u64::MAX);
        self.last_seq = record.body.seq;
        self.last_hash.clone_from(&record.hash);
        self.last_was_anchor = kind == "audit.anchor";
        self.unsynced += 1;
        if self.unsynced >= self.options.fsync_every.max(1) {
            self.sync();
        }
        Some(record)
    }

    fn sync(&mut self) {
        if self.unsynced == 0 {
            return;
        }
        if let Err(err) = self.file.sync_data() {
            let diagnostic = not_writable(&self.path, &format!("fsync failed: {err}"));
            self.report(diagnostic);
        }
        self.unsynced = 0;
        self.last_sync = Instant::now();
    }

    fn report(&self, diagnostic: Diagnostic) {
        tracing::warn!(code = %diagnostic.code, why = %diagnostic.why, "audit");
        let _ = self.diagnostics.send(diagnostic);
    }
}

/// Legt das Verzeichnis an und zieht es auf `0700`.
fn ensure_log_dir(path: &Path) -> Result<(), Diagnostic> {
    let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) else {
        return Ok(());
    };
    let result = (|| -> io::Result<()> {
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent)?;
        }
        match DirBuilder::new().mode(LOG_DIR_MODE).create(dir) {
            Ok(()) => {}
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
        let meta = fs::metadata(dir)?;
        if !meta.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "exists but is not a directory",
            ));
        }
        if meta.permissions().mode() & 0o077 != 0 {
            fs::set_permissions(dir, fs::Permissions::from_mode(LOG_DIR_MODE))?;
        }
        Ok(())
    })();
    result.map_err(|err| not_writable(dir, &err.to_string()))
}

/// Öffnet die Datei zum Anhängen, ohne einem Symlink zu folgen.
fn open_log(path: &Path) -> Result<File, Diagnostic> {
    let file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .mode(LOG_MODE)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)
        .map_err(|err| not_writable(path, &format!("cannot open it: {err}")))?;
    let meta = file
        .metadata()
        .map_err(|err| not_writable(path, &format!("cannot inspect it: {err}")))?;
    if !meta.is_file() {
        return Err(not_writable(path, "it is not a regular file"));
    }
    if meta.permissions().mode() & 0o077 != 0 {
        file.set_permissions(fs::Permissions::from_mode(LOG_MODE))
            .map_err(|err| not_writable(path, &format!("cannot set 0600: {err}")))?;
    }
    Ok(file)
}

/// Nimmt die exklusive Sperre, ohne zu warten.
fn lock(file: &File, path: &Path) -> Result<(), Diagnostic> {
    match rustix::fs::flock(file, rustix::fs::FlockOperation::NonBlockingLockExclusive) {
        Ok(()) => Ok(()),
        Err(errno) if errno == rustix::io::Errno::WOULDBLOCK => {
            Err(Diagnostic::builder(AUDIT_004, Severity::Blocking)
                .why(format!(
                    "another process holds the lock on {}; two daemons never write one chain",
                    path.display()
                ))
                .fix(FixAction::CopyCommand("humanitl daemon status".to_owned()))
                .build())
        }
        Err(errno) => Err(not_writable(path, &format!("cannot lock it: {errno}"))),
    }
}

fn byte_at(file: &File, at: u64) -> io::Result<u8> {
    let mut byte = [0_u8; 1];
    file.read_exact_at(&mut byte, at)?;
    Ok(byte[0])
}

/// Der Anfang der Zeile, die vor `end` endet: die Stelle hinter dem letzten
/// `\n` vor `end`, oder `0`.
fn line_start(file: &File, end: u64) -> io::Result<u64> {
    let mut chunk_end = end;
    let mut buffer = vec![0_u8; usize::try_from(TAIL_CHUNK).unwrap_or(65_536)];
    while chunk_end > 0 {
        let chunk_start = chunk_end.saturating_sub(TAIL_CHUNK);
        let size = usize::try_from(chunk_end - chunk_start).unwrap_or(0);
        let slice = &mut buffer[..size];
        file.read_exact_at(slice, chunk_start)?;
        if let Some(at) = slice.iter().rposition(|&byte| byte == b'\n') {
            return Ok(chunk_start + u64::try_from(at).unwrap_or(0) + 1);
        }
        chunk_end = chunk_start;
    }
    Ok(0)
}

/// Verschiebt den Rest hinter dem letzten `\n` in eine eigene Datei und kürzt
/// das Log auf den letzten vollständigen Record.
fn set_aside_torn_tail(
    file: &File,
    path: &Path,
    len: u64,
) -> Result<(u64, Diagnostic), Diagnostic> {
    let start = line_start(file, len).map_err(|err| unreadable(path, &err))?;
    let mut aside = path.as_os_str().to_os_string();
    aside.push(format!(
        ".corrupt-{}",
        Utc::now().format("%Y%m%dT%H%M%S%.6fZ")
    ));
    let aside = PathBuf::from(aside);
    let moved = (|| -> io::Result<()> {
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(LOG_MODE)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&aside)?;
        let mut at = start;
        let mut buffer = vec![0_u8; usize::try_from(TAIL_CHUNK).unwrap_or(65_536)];
        while at < len {
            let size = usize::try_from((len - at).min(TAIL_CHUNK)).unwrap_or(0);
            file.read_exact_at(&mut buffer[..size], at)?;
            out.write_all(&buffer[..size])?;
            at += u64::try_from(size).unwrap_or(u64::MAX);
        }
        out.sync_all()?;
        file.set_len(start)?;
        file.sync_all()
    })();
    moved.map_err(|err| {
        not_writable(
            path,
            &format!(
                "cannot move the torn last line to {}: {err}",
                aside.display()
            ),
        )
    })?;
    let note = Diagnostic::builder(AUDIT_002, Severity::Warning)
        .why(format!(
            "{} ended in the middle of a line; the {} bytes after the last complete record were \
             moved to {}, and the chain continues after that record without a gap",
            path.display(),
            len - start,
            aside.display()
        ))
        .build();
    Ok((start, note))
}

/// Liest den letzten vollständigen Record und prüft ihn gegen den Schlüssel.
fn last_record(
    file: &File,
    path: &Path,
    len: u64,
    key: &[u8; KEY_LEN],
) -> Result<Head, Diagnostic> {
    // `len - 1` ist das `\n` der letzten Zeile.
    let end = len - 1;
    let start = line_start(file, end).map_err(|err| unreadable(path, &err))?;
    if end - start > MAX_LINE_BYTES {
        return Err(broken_end(
            path,
            &format!(
                "the last line is {} bytes long, more than any record",
                end - start
            ),
        ));
    }
    let mut line = vec![0_u8; usize::try_from(end - start).unwrap_or(0)];
    file.read_exact_at(&mut line, start)
        .map_err(|err| unreadable(path, &err))?;
    let record = AuditRecord::from_line(&line)
        .map_err(|err| broken_end(path, &format!("the last line is no record: {err}")))?;
    let canonical = record.to_line().ok();
    if canonical.as_deref() != Some(line.as_slice()) {
        return Err(broken_end(path, "the last record is not in canonical form"));
    }
    let Ok(hash) = record.body.hash() else {
        return Err(broken_end(
            path,
            "the last record holds data that is not canonical",
        ));
    };
    if hex::encode(hash) != record.hash {
        return Err(broken_end(
            path,
            &format!(
                "the hash of the last record (seq {}) does not match its fields",
                record.body.seq
            ),
        ));
    }
    if !mac_matches(key, &hash, &record.mac) {
        return Err(broken_end(
            path,
            &format!(
                "the MAC of the last record (seq {}) does not match the audit key; the key was \
                 replaced or the chain was rebuilt without it",
                record.body.seq
            ),
        ));
    }
    Ok(Head {
        seq: record.body.seq,
        hash: record.hash,
    })
}

/// Die Lücke zwischen dem Ende des Logs und dem letzten Anker dahinter.
struct Gap {
    /// Wo das Log beim Öffnen endete.
    log_seq: u64,
    /// Der letzte Anker der zweiten Ablage.
    anchor_seq: u64,
    /// Sein Hash: der `prev` des ersten Records hinter der Lücke.
    anchor_hash: String,
}

/// Ein Anker unter der Nummer des Endes muss passen. Liegen Anker dahinter,
/// kommt die Lücke zurück, hinter deren letztem Anker die Kette weiterläuft.
fn check_anchors(path: &Path, head: &Head, anchors: &[Anchor]) -> Result<Option<Gap>, Diagnostic> {
    if let Some(mismatch) = anchors
        .iter()
        .find(|anchor| anchor.seq == head.seq && anchor.hash != head.hash)
    {
        return Err(broken_end(
            path,
            &format!(
                "the anchor at seq {} names another hash than the last record",
                mismatch.seq
            ),
        ));
    }
    Ok(anchors
        .iter()
        .filter(|anchor| anchor.seq > head.seq)
        .max_by_key(|anchor| anchor.seq)
        .map(|last| Gap {
            log_seq: head.seq,
            anchor_seq: last.seq,
            anchor_hash: last.hash.clone(),
        }))
}

fn resumed_note(path: &Path, gap: &Gap) -> Diagnostic {
    Diagnostic::builder(AUDIT_007, Severity::Warning)
        .why(format!(
            "{} ends at seq {}, but the anchor table names seq {}: the log was cut, replaced or \
             set aside after that anchor was taken. The chain continues after that anchor with \
             an audit.resumed record; verify keeps reporting the gap as a break, and the anchors \
             stay in the table audit_anchors of the recording database as evidence",
            path.display(),
            gap.log_seq,
            gap.anchor_seq
        ))
        .build()
}

fn broken_end(path: &Path, why: &str) -> Diagnostic {
    Diagnostic::builder(AUDIT_001, Severity::Error)
        .why(format!(
            "{}: {why}; the writer does not append to a chain whose end does not hold",
            path.display()
        ))
        .fix(set_aside_fix(path))
        .build()
}

fn not_writable(path: &Path, why: &str) -> Diagnostic {
    let quoted = shell_quote(&path.display().to_string());
    Diagnostic::builder(AUDIT_006, Severity::Error)
        .why(format!("the audit log {}: {why}", path.display()))
        .fix(FixAction::CopyCommand(format!(
            "ls -ld {quoted} && df -h {quoted}"
        )))
        .build()
}

fn unreadable(path: &Path, err: &io::Error) -> Diagnostic {
    not_writable(path, &format!("cannot read it: {err}"))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use serde_json::json;

    use super::checked_data;

    #[test]
    fn a_float_becomes_a_placeholder_and_a_finding() {
        let (data, problem) = checked_data("flow.responded", json!({"duration_ms": 1.5}));
        assert_eq!(data, json!({"error": "non_canonical"}));
        let problem = problem.expect("a finding");
        assert_eq!(problem.code.as_str(), "AUDIT_003");
        assert!(problem.why.contains("flow.responded"), "{}", problem.why);

        let (data, problem) = checked_data("flow.responded", json!({"duration_ms": 15}));
        assert_eq!(data, json!({"duration_ms": 15}));
        assert!(problem.is_none());
    }
}
