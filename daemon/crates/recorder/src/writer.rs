//! Der Schreib-Thread.
//!
//! `rusqlite::Connection` ist `!Sync`, also gehört sie genau einem Thread. Alle
//! Schreibvorgänge kommen als [`WriterCmd`] über einen Kanal an; das Handle
//! [`Recorder`](crate::Recorder) ist deshalb `Clone + Send + Sync`, ohne dass
//! irgendwo eine Verbindung geteilt würde.
//!
//! # Bündeln
//!
//! Ein Flow schreibt sechs bis acht Zeilen. Jede einzeln zu committen hieße,
//! bei jedem Ereignis auf die Platte zu warten. Der Thread öffnet die
//! Transaktion deshalb beim ersten Kommando und schließt sie nach
//! [`BATCH_INTERVAL`] oder [`BATCH_COMMANDS`], je nachdem, was zuerst eintritt.
//! Ein Absturz kostet dann höchstens die letzten 50 ms; Bodies liegen zu dem
//! Zeitpunkt schon als Datei im Blob-Speicher, also fehlt danach eine Zeile,
//! nie ein halber Body.
//!
//! # Fehler
//!
//! Kein `unwrap` und kein `panic`: der Thread stirbt nicht, wenn eine Zeile
//! nicht schreibbar ist. Der Fehler wird zu einem [`Diagnostic`] mit
//! `RECORDER_003` und geht über einen `broadcast`-Kanal an den Daemon, der ihn
//! in den Ereignisstrom stellt (`FlowEvent::Diagnostic`). So ist eine Lücke in
//! der Aufzeichnung sichtbar, statt still zu bleiben.
//!
//! Kein Fehlertext trägt je Inhalte einer Nachricht: nur Kennungen, Spalten,
//! Pfade und die Meldung von `SQLite`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use bytes::Bytes;
use humanitl_core::ids::SandboxId;
use humanitl_core::{
    BlockReason, Decision, DecisionSource, Diagnostic, Finding, FixAction, FlowEvent, FlowId,
    HeaderMap, HttpRequest, Scheme, SessionId,
};
use rusqlite::Connection;
use tokio::sync::{broadcast, oneshot};

use crate::blob::BlobStore;
use crate::error::{RecorderError, storage_failed};
use crate::hostkey::host_key;
use crate::types::{Dir, PurgeReport, SessionMeta, millis};

/// So lange bleibt eine Transaktion höchstens offen.
pub const BATCH_INTERVAL: Duration = Duration::from_millis(50);

/// So viele Kommandos passen höchstens in eine Transaktion.
pub const BATCH_COMMANDS: usize = 100;

/// So lange bleibt ein angemeldeter Blob vor dem Aufräumen geschützt.
///
/// Ein Blob wird geschrieben, bevor seine Zeile entsteht (siehe `blob`-Modul).
/// Zwischen beidem darf ein Aufräumlauf ihn nicht für verwaist halten. Die
/// Anmeldung fällt nach dieser Frist von selbst, damit ein abgebrochener
/// Vorgang den Schutz nicht ewig hält.
pub const RESERVATION_GRACE: Duration = Duration::from_secs(300);

/// Der Kanal zum Schreib-Thread samt dem Thread selbst.
///
/// Alle Kopien des [`Recorder`](crate::Recorder) und jeder offene
/// [`ResponseSink`](crate::ResponseSink) halten denselben `Arc` darauf. Fällt
/// der letzte, schließt sich der Kanal, der Thread schreibt seine letzte
/// Transaktion und wird abgewartet: danach steht alles auf der Platte, ohne
/// dass jemand daran denken müsste, `flush` als letztes aufzurufen.
#[derive(Debug)]
pub struct WriterHandle {
    tx: Sender<WriterCmd>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl WriterHandle {
    /// Verbindet Kanal und Thread.
    pub fn new(tx: Sender<WriterCmd>, join: JoinHandle<()>) -> Self {
        Self {
            tx,
            join: Mutex::new(Some(join)),
        }
    }

    /// Schickt ein Kommando an den Schreib-Thread.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] mit `RECORDER_003`, wenn der Thread nicht
    /// mehr da ist. Dann wird nichts mehr aufgezeichnet, und das ist ein
    /// Befund, kein stilles Nichts.
    pub fn send(&self, cmd: WriterCmd) -> Result<(), RecorderError> {
        self.tx.send(cmd).map_err(|_gone| {
            storage_failed(
                "the recorder thread is gone; nothing is being recorded any more. Restart the \
                 daemon so that the guarantee \"everything is recorded\" holds again",
            )
            .with_fix(FixAction::CopyCommand(
                "systemctl --user restart humanitld".to_owned(),
            ))
        })
    }
}

impl Drop for WriterHandle {
    fn drop(&mut self) {
        // Erst den Kanal schließen, dann warten: der Thread verlässt seine
        // Schleife über `Disconnected` und committet dabei die offene
        // Transaktion.
        let (dead, rx) = std::sync::mpsc::channel();
        drop(rx);
        drop(core::mem::replace(&mut self.tx, dead));
        let handle = self
            .join
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(handle) = handle {
            let _ignored = handle.join();
        }
    }
}

/// Ein Auftrag an den Schreib-Thread.
#[derive(Debug)]
pub enum WriterCmd {
    /// Eine Sitzung beginnt; sie wird ab jetzt für neue Flows eingetragen.
    StartSession(Box<SessionMeta>),
    /// Eine Sitzung endet.
    EndSession {
        /// Die Sitzung.
        id: SessionId,
        /// Wann, in Unix-Millisekunden.
        at: i64,
    },
    /// Ein Ereignis des Flows.
    Event(Box<FlowEvent>),
    /// Eine Nachricht samt Kopfzeilen und Body-Verweis.
    Message(Box<MessageWrite>),
    /// Die Funde eines Flows.
    Findings {
        /// Der Flow.
        flow: FlowId,
        /// Die Funde in der Reihenfolge der Detektoren.
        findings: Vec<Finding>,
    },
    /// Apex und Katalog-Kennung eines Flows.
    Domain {
        /// Der Flow.
        flow: FlowId,
        /// Der Apex nach der Public Suffix List.
        apex: Option<String>,
        /// Die Kennung im Domain-Katalog.
        catalog_id: Option<String>,
    },
    /// Woran ein Flow gescheitert ist (`flows.error`).
    ///
    /// Für den Fall, den kein Ereignis trägt: der Client in der Sandbox bricht
    /// den TLS-Handschlag zum Proxy ab (HUM-045). Der Weg nach draußen schreibt
    /// die Spalte dagegen aus [`FlowEvent::Failed`].
    FlowError {
        /// Der Flow.
        flow: FlowId,
        /// Der feste Bezeichner des Grundes, zum Beispiel `tls_handshake_failed`.
        error: String,
    },
    /// Eine Regel, wie sie zum Zeitpunkt der Entscheidung aussah.
    Rule {
        /// Die Id der Regel.
        id: String,
        /// Die Regel als `YAML`.
        yaml: String,
        /// Wann sie zuerst gesehen wurde, in Unix-Millisekunden.
        at: i64,
    },
    /// Die Zusammenfassung eines Sandbox-Laufs (HUM-043).
    SessionSummary {
        /// Die Sitzung des Daemons.
        session: SessionId,
        /// Der Sandbox-Lauf innerhalb dieser Sitzung.
        sandbox: SandboxId,
        /// Wann, in Unix-Millisekunden.
        at: i64,
        /// Die Zusammenfassung als `JSON`. Die Struktur gehört
        /// `humanitl-sandbox`; diese Crate speichert den Text.
        json: String,
    },
    /// Eine Regel gibt es nicht mehr.
    RuleDeleted {
        /// Die Id der Regel.
        id: String,
        /// Wann, in Unix-Millisekunden.
        at: i64,
    },
    /// Alles löschen, was älter ist als dieser Zeitpunkt.
    Purge {
        /// Unix-Millisekunden; alles davor fällt weg.
        before: i64,
        /// Wohin der Bericht geht.
        reply: oneshot::Sender<Result<PurgeReport, RecorderError>>,
    },
    /// Ein Blob ist geschrieben oder wird gerade geschrieben; seine Zeile
    /// kommt gleich. Bis dahin fasst ihn kein Aufräumlauf an.
    ReserveBlob([u8; 32]),
    /// Statistiken für den Abfrageplaner neu erheben.
    Analyze(oneshot::Sender<Result<(), RecorderError>>),
    /// Offene Transaktion schließen und Bescheid geben.
    Flush(oneshot::Sender<()>),
}

/// Eine Nachricht, fertig zum Schreiben.
///
/// Der Body ist zu diesem Zeitpunkt entweder klein genug für die Datenbank
/// (`inline`) oder liegt schon als Datei im Blob-Speicher (`blob`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageWrite {
    /// Der Flow.
    pub flow: FlowId,
    /// Richtung.
    pub dir: Dir,
    /// Die Kopfzeilen als `[["name","value"],…]`.
    pub headers_json: String,
    /// Der Wert aus `Content-Type`.
    pub content_type: Option<String>,
    /// Der Wert aus `Content-Encoding`.
    pub content_encoding: Option<String>,
    /// Der Body, falls er in der Datenbank steht.
    pub inline: Option<Bytes>,
    /// Der Verweis in den Blob-Speicher, falls er dort steht.
    pub blob: Option<[u8; 32]>,
    /// Länge des Bodys, wie er gesendet wurde.
    pub size: u64,
    /// Wahr, wenn nur ein Anfang gespeichert wurde.
    pub truncated: bool,
    /// Der Status der Antwort; nur bei [`Dir::Response`] gesetzt.
    pub status: Option<u16>,
}

/// Der Zustand des Schreib-Threads.
pub struct Writer {
    conn: Connection,
    db: PathBuf,
    blobs: Arc<BlobStore>,
    diagnostics: broadcast::Sender<Diagnostic>,
    open: bool,
    opened_at: Instant,
    pending: usize,
    restricted: bool,
    reserved: HashMap<[u8; 32], Instant>,
    held: HashMap<FlowId, i64>,
    session: Option<SessionId>,
}

impl Writer {
    /// Ein Schreiber auf dieser Verbindung.
    pub fn new(
        conn: Connection,
        db: &Path,
        blobs: Arc<BlobStore>,
        diagnostics: broadcast::Sender<Diagnostic>,
    ) -> Self {
        Self {
            conn,
            db: db.to_path_buf(),
            blobs,
            diagnostics,
            open: false,
            opened_at: Instant::now(),
            pending: 0,
            restricted: false,
            reserved: HashMap::new(),
            held: HashMap::new(),
            session: None,
        }
    }

    /// Läuft, bis alle Handles fallen gelassen wurden.
    pub fn run(mut self, rx: &Receiver<WriterCmd>) {
        loop {
            let cmd = if self.open {
                let left = BATCH_INTERVAL.saturating_sub(self.opened_at.elapsed());
                match rx.recv_timeout(left) {
                    Ok(cmd) => cmd,
                    Err(RecvTimeoutError::Timeout) => {
                        self.commit();
                        continue;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            } else {
                match rx.recv() {
                    Ok(cmd) => cmd,
                    Err(_disconnected) => break,
                }
            };

            match cmd {
                WriterCmd::Flush(reply) => {
                    self.commit();
                    let _ignored = reply.send(());
                }
                WriterCmd::Purge { before, reply } => {
                    self.commit();
                    let report = self.purge(before);
                    let _ignored = self.analyze();
                    let _ignored = reply.send(report);
                }
                WriterCmd::Analyze(reply) => {
                    self.commit();
                    let _ignored = reply.send(self.analyze());
                }
                other => {
                    self.begin();
                    self.dispatch(other);
                    self.pending += 1;
                    if self.pending >= BATCH_COMMANDS {
                        self.commit();
                    }
                }
            }
        }
        self.commit();
    }

    /// Öffnet die Transaktion, falls noch keine offen ist.
    fn begin(&mut self) {
        if self.open {
            return;
        }
        match self.conn.execute_batch("BEGIN;") {
            Ok(()) => {
                self.open = true;
                self.opened_at = Instant::now();
                self.pending = 0;
            }
            Err(err) => self.report(storage_failed(format!(
                "could not begin a recorder transaction ({err})"
            ))),
        }
    }

    /// Schließt die offene Transaktion.
    fn commit(&mut self) {
        if !self.open {
            return;
        }
        self.open = false;
        self.pending = 0;
        if let Err(err) = self.conn.execute_batch("COMMIT;") {
            let _ignored = self.conn.execute_batch("ROLLBACK;");
            self.report(storage_failed(format!(
                "could not commit a recorder transaction ({err}); the rows of this batch are lost"
            )));
        }
        self.restrict_once();
    }

    /// Setzt die Rechte von `-wal` und `-shm` nach der ersten Transaktion.
    ///
    /// Die beiden Dateien entstehen erst mit einer Transaktion, und zwar mit
    /// der Standardmaske des Prozesses. Sie tragen den frischesten Teil der
    /// Aufzeichnung; nach dem ersten Commit dieses Threads werden sie deshalb
    /// auf `0600` gesetzt. Ein Fehler dabei ist ein Befund, kein Abbruch: die
    /// Aufzeichnung läuft weiter, aber der Nutzer erfährt, dass die Datei
    /// offener liegt als zugesagt.
    fn restrict_once(&mut self) {
        if self.restricted {
            return;
        }
        self.restricted = true;
        if let Err(err) = crate::schema::restrict(&self.db) {
            self.report(err);
        }
    }

    /// Führt ein Kommando aus und meldet, was dabei schiefging.
    fn dispatch(&mut self, cmd: WriterCmd) {
        let result = match cmd {
            WriterCmd::StartSession(meta) => self.start_session(&meta),
            WriterCmd::EndSession { id, at } => self.end_session(id, at),
            WriterCmd::Event(event) => self.on_event(&event),
            WriterCmd::Message(message) => self.write_message(&message),
            WriterCmd::Findings { flow, findings } => self.write_findings(flow, &findings),
            WriterCmd::Domain {
                flow,
                apex,
                catalog_id,
            } => self.write_domain(flow, apex.as_deref(), catalog_id.as_deref()),
            WriterCmd::FlowError { flow, error } => self.write_flow_error(flow, &error),
            WriterCmd::Rule { id, yaml, at } => self.write_rule(&id, &yaml, at),
            WriterCmd::SessionSummary {
                session,
                sandbox,
                at,
                json,
            } => self.write_session_summary(session, sandbox, at, &json),
            WriterCmd::RuleDeleted { id, at } => self.write_rule_deleted(&id, at),
            WriterCmd::ReserveBlob(sha256) => {
                self.reserved.insert(sha256, Instant::now());
                Ok(())
            }
            WriterCmd::Flush(_) | WriterCmd::Purge { .. } | WriterCmd::Analyze(_) => Ok(()),
        };
        if let Err(err) = result {
            self.report(err);
        }
    }

    /// Schickt einen Befund an den Daemon.
    fn report(&self, err: RecorderError) {
        let diagnostic = err.into_diagnostic();
        tracing::warn!(code = diagnostic.code.as_str(), why = %diagnostic.why, "recorder");
        let _ignored = self.diagnostics.send(diagnostic);
    }

    /// Trägt eine Sitzung ein und macht sie zur laufenden.
    fn start_session(&mut self, meta: &SessionMeta) -> Result<(), RecorderError> {
        let id = meta.id.to_string();
        self.conn
            .execute(
                "INSERT INTO sessions (id, started_at, ended_at, sandbox_profile, llm_endpoint, \
                 work_dir, agent) VALUES (?1, ?2, NULL, ?3, ?4, ?5, ?6) \
                 ON CONFLICT(id) DO UPDATE SET started_at = excluded.started_at, \
                 sandbox_profile = excluded.sandbox_profile, llm_endpoint = excluded.llm_endpoint, \
                 work_dir = excluded.work_dir, agent = excluded.agent",
                rusqlite::params![
                    id,
                    millis(meta.started_at),
                    meta.sandbox_profile,
                    meta.llm_endpoint,
                    meta.work_dir,
                    meta.agent,
                ],
            )
            .map_err(|err| storage_failed(format!("could not record the session {id} ({err})")))?;
        self.session = Some(meta.id);
        Ok(())
    }

    /// Schreibt das Ende einer Sitzung.
    fn end_session(&mut self, id: SessionId, at: i64) -> Result<(), RecorderError> {
        let key = id.to_string();
        self.conn
            .execute(
                "UPDATE sessions SET ended_at = ?2 WHERE id = ?1",
                rusqlite::params![key, at],
            )
            .map_err(|err| storage_failed(format!("could not end the session {key} ({err})")))?;
        if self.session == Some(id) {
            self.session = None;
        }
        Ok(())
    }

    /// Legt die Zeile eines angekommenen Flows an.
    ///
    /// `seq` wird in derselben Anweisung berechnet, in der die Zeile entsteht:
    /// ein getrenntes `SELECT MAX(seq)` wäre ein Wettlauf, sobald zwei Flows
    /// gleichzeitig ankommen.
    fn on_received(
        &self,
        flow_id: &FlowId,
        at: SystemTime,
        request: &HttpRequest,
    ) -> Result<(), RecorderError> {
        let Some(session) = self.session else {
            return Err(storage_failed(format!(
                "the flow {flow_id} arrived before any session was started; call \
                 Recorder::start_session once per sandbox session"
            )));
        };
        let upgrade = upgrade_of(&request.headers, request.scheme);
        let host = request.authority.host.to_string();
        let host_display = request.authority.host.display();
        let host_rev = host_key(&host);
        self.conn
            .execute(
                "INSERT INTO flows (id, session_id, seq, ts, method, scheme, host, host_display, \
                 host_rev, port, path, upgrade, state, request_size) VALUES (?1, ?2, \
                 (SELECT COALESCE(MAX(seq), 0) + 1 FROM flows WHERE session_id = ?2), ?3, ?4, ?5, \
                 ?6, ?7, ?8, ?9, ?10, ?11, 'received', ?12)",
                rusqlite::params![
                    flow_id.to_string(),
                    session.to_string(),
                    millis(at),
                    request.method.as_str(),
                    request.scheme.as_str(),
                    host,
                    host_display,
                    host_rev,
                    i64::from(request.authority.port),
                    request.path_and_query,
                    upgrade,
                    i64::try_from(request.body.size).unwrap_or(i64::MAX),
                ],
            )
            .map(|_rows| ())
            .map_err(|err| storage_failed(format!("could not record the flow {flow_id} ({err})")))
    }

    /// Schreibt die Entscheidung samt ihrer Herkunft fort.
    fn on_decided(
        &mut self,
        flow_id: &FlowId,
        at: SystemTime,
        decision: &Decision,
        source: DecisionSource,
    ) -> Result<(), RecorderError> {
        let held_ms = self.held.remove(flow_id).map(|held| millis(at) - held);
        let reason = decision.block_reason();
        let rule = rule_of(source, reason);
        self.update(
            flow_id,
            "UPDATE flows SET state = 'decided', decision = ?2, block_reason = ?3, \
             rule_id = COALESCE(?4, rule_id), held_ms = COALESCE(?5, held_ms), edited = ?6, \
             passthrough = MAX(passthrough, ?7) WHERE id = ?1",
            rusqlite::params![
                flow_id.to_string(),
                decision.as_str(),
                reason.map(BlockReason::as_str),
                rule,
                held_ms,
                i64::from(matches!(decision, Decision::AllowEdited { .. })),
                i64::from(source == DecisionSource::Passthrough),
            ],
        )
    }

    /// Schreibt fort, was ein Ereignis über den Flow sagt.
    fn on_event(&mut self, event: &FlowEvent) -> Result<(), RecorderError> {
        match event {
            FlowEvent::Received {
                flow_id,
                at,
                request,
            } => self.on_received(flow_id, *at, request),
            FlowEvent::Analyzed {
                flow_id, findings, ..
            } => self.update(
                flow_id,
                "UPDATE flows SET state = 'analyzed', findings_count = ?2 WHERE id = ?1",
                rusqlite::params![
                    flow_id.to_string(),
                    i64::try_from(findings.len()).unwrap_or(i64::MAX)
                ],
            ),
            FlowEvent::Held { flow_id, at, .. } => {
                self.held.insert(*flow_id, millis(*at));
                self.update(
                    flow_id,
                    "UPDATE flows SET state = 'held' WHERE id = ?1",
                    rusqlite::params![flow_id.to_string()],
                )
            }
            FlowEvent::Decided {
                flow_id,
                at,
                decision,
                source,
            } => self.on_decided(flow_id, *at, decision, *source),
            FlowEvent::Forwarded { flow_id, .. } => self.update(
                flow_id,
                "UPDATE flows SET state = 'forwarded' WHERE id = ?1",
                rusqlite::params![flow_id.to_string()],
            ),
            FlowEvent::ResponseHeaders {
                flow_id, status, ..
            } => self.update(
                flow_id,
                "UPDATE flows SET state = 'responded', status = ?2 WHERE id = ?1",
                rusqlite::params![flow_id.to_string(), i64::from(*status)],
            ),
            // `error` sagt, woran der Weg nach draußen gescheitert ist. Ein
            // schon gesetzter Grund bleibt stehen: Der erste ist der, der den
            // Flow beendet hat, jeder weitere wäre nur seine Folge.
            FlowEvent::Failed { flow_id, error, .. } => self.update(
                flow_id,
                "UPDATE flows SET state = 'failed', error = COALESCE(error, ?2) WHERE id = ?1",
                rusqlite::params![flow_id.to_string(), error.to_string()],
            ),
            FlowEvent::TimedOut { flow_id, at } => {
                let held_ms = self.held.remove(flow_id).map(|held| millis(*at) - held);
                self.update(
                    flow_id,
                    "UPDATE flows SET state = 'decided', decision = 'timed_out', \
                     block_reason = 'timeout', held_ms = COALESCE(?2, held_ms) WHERE id = ?1",
                    rusqlite::params![flow_id.to_string(), held_ms],
                )
            }
            FlowEvent::Recorded { flow_id, at } => {
                self.held.remove(flow_id);
                self.update(
                    flow_id,
                    "UPDATE flows SET state = 'recorded', duration_ms = ?2 - ts WHERE id = ?1",
                    rusqlite::params![flow_id.to_string(), millis(*at)],
                )
            }
            // Nichts davon gehört zu einer Zeile in `flows`: Der Zähler
            // eines Antwortstücks steht dort schon, ein Rückstand ist kein
            // Zustand, ein Befund hat seine eigene Tabelle, und die Bitte des
            // Agenten aus `humanitl.internal/ask` ist gar kein Flow
            // (HUM-073).
            FlowEvent::ResponseChunk { .. }
            | FlowEvent::Lagged { .. }
            | FlowEvent::Diagnostic { .. }
            | FlowEvent::AgentAsk { .. } => Ok(()),
        }
    }

    /// Führt ein `UPDATE` auf einer Flow-Zeile aus.
    fn update(
        &self,
        flow: &FlowId,
        sql: &str,
        params: &[&dyn rusqlite::ToSql],
    ) -> Result<(), RecorderError> {
        self.conn
            .execute(sql, params)
            .map(|_rows| ())
            .map_err(|err| storage_failed(format!("could not update the flow {flow} ({err})")))
    }

    /// Schreibt eine Nachricht und, bei einer Antwort, ihre Kennzahlen.
    fn write_message(&mut self, message: &MessageWrite) -> Result<(), RecorderError> {
        if let Some(sha256) = message.blob {
            // Die Zeile steht jetzt; der Blob braucht keinen Schutz mehr.
            self.reserved.remove(&sha256);
        }
        let flow = message.flow.to_string();
        self.conn
            .execute(
                "INSERT INTO messages (flow_id, dir, headers_json, content_type, \
                 content_encoding, body_inline, blob_sha256, size, truncated) VALUES (?1, ?2, ?3, \
                 ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT(flow_id, dir) DO UPDATE SET \
                 headers_json = excluded.headers_json, content_type = excluded.content_type, \
                 content_encoding = excluded.content_encoding, body_inline = excluded.body_inline, \
                 blob_sha256 = excluded.blob_sha256, size = excluded.size, \
                 truncated = excluded.truncated",
                rusqlite::params![
                    flow,
                    message.dir.as_str(),
                    message.headers_json,
                    message.content_type,
                    message.content_encoding,
                    message.inline.as_ref().map(AsRef::as_ref),
                    message.blob.as_ref().map(<[u8; 32]>::as_slice),
                    i64::try_from(message.size).unwrap_or(i64::MAX),
                    i64::from(message.truncated),
                ],
            )
            .map_err(|err| {
                storage_failed(format!(
                    "could not record the {} of the flow {flow} ({err})",
                    message.dir
                ))
            })?;

        if matches!(message.dir, Dir::Request | Dir::RequestEdited) {
            // `FlowEvent::Received` entsteht, bevor der Body gelesen ist; seine
            // Größe steht dort noch auf null. Erst hier ist sie bekannt, und
            // erst hier stimmen `FlowSummary::request_size` und die Sortierung
            // nach Größe. Die bearbeitete Anfrage überschreibt die ursprüngliche,
            // weil sie die ist, die hinausgeht.
            self.conn
                .execute(
                    "UPDATE flows SET request_size = ?2 WHERE id = ?1",
                    rusqlite::params![flow, i64::try_from(message.size).unwrap_or(i64::MAX)],
                )
                .map_err(|err| {
                    storage_failed(format!(
                        "could not record the request size of the flow {flow} ({err})"
                    ))
                })?;
        }

        if message.dir == Dir::Response {
            self.conn
                .execute(
                    "UPDATE flows SET response_size = ?2, status = COALESCE(?3, status) \
                     WHERE id = ?1",
                    rusqlite::params![
                        flow,
                        i64::try_from(message.size).unwrap_or(i64::MAX),
                        message.status.map(i64::from),
                    ],
                )
                .map_err(|err| {
                    storage_failed(format!(
                        "could not record the response size of the flow {flow} ({err})"
                    ))
                })?;
        }
        Ok(())
    }

    /// Schreibt die Funde eines Flows und ihre Anzahl.
    fn write_findings(&self, flow: FlowId, findings: &[Finding]) -> Result<(), RecorderError> {
        let key = flow.to_string();
        for (index, finding) in findings.iter().enumerate() {
            self.conn
                .execute(
                    "INSERT INTO findings (flow_id, idx, kind, location, span_start, span_end, \
                     tier, value_hash, display_prefix, resolved) VALUES (?1, ?2, ?3, ?4, ?5, ?6, \
                     ?7, ?8, ?9, NULL) ON CONFLICT(flow_id, idx) DO UPDATE SET \
                     kind = excluded.kind, location = excluded.location, \
                     span_start = excluded.span_start, span_end = excluded.span_end, \
                     tier = excluded.tier, value_hash = excluded.value_hash, \
                     display_prefix = excluded.display_prefix",
                    rusqlite::params![
                        key,
                        i64::try_from(index).unwrap_or(i64::MAX),
                        finding.kind.to_string(),
                        finding.location.to_string(),
                        i64::try_from(finding.span.start).unwrap_or(i64::MAX),
                        i64::try_from(finding.span.end).unwrap_or(i64::MAX),
                        finding.tier.as_str(),
                        finding.value_hash.as_slice(),
                        finding.display_prefix,
                    ],
                )
                .map_err(|err| {
                    storage_failed(format!(
                        "could not record finding {index} of the flow {key} ({err})"
                    ))
                })?;
        }
        self.conn
            .execute(
                "UPDATE flows SET findings_count = ?2 WHERE id = ?1",
                rusqlite::params![key, i64::try_from(findings.len()).unwrap_or(i64::MAX)],
            )
            .map_err(|err| {
                storage_failed(format!(
                    "could not record the finding count of the flow {key} ({err})"
                ))
            })?;
        Ok(())
    }

    /// Trägt Apex und Katalog-Kennung nach.
    fn write_domain(
        &self,
        flow: FlowId,
        apex: Option<&str>,
        catalog_id: Option<&str>,
    ) -> Result<(), RecorderError> {
        let key = flow.to_string();
        self.conn
            .execute(
                "UPDATE flows SET apex = COALESCE(?2, apex), catalog_id = COALESCE(?3, catalog_id) \
                 WHERE id = ?1",
                rusqlite::params![key, apex, catalog_id],
            )
            .map(|_rows| ())
            .map_err(|err| {
                storage_failed(format!(
                    "could not record the domain of the flow {key} ({err})"
                ))
            })
    }

    /// Trägt nach, woran ein Flow gescheitert ist.
    ///
    /// Wie bei [`FlowEvent::Failed`] bleibt ein schon gesetzter Grund stehen;
    /// der erste ist der, der den Flow beendet hat.
    fn write_flow_error(&self, flow: FlowId, error: &str) -> Result<(), RecorderError> {
        let key = flow.to_string();
        self.conn
            .execute(
                "UPDATE flows SET error = COALESCE(error, ?2) WHERE id = ?1",
                rusqlite::params![key, error],
            )
            .map(|_rows| ())
            .map_err(|err| {
                storage_failed(format!(
                    "could not record the error of the flow {key} ({err})"
                ))
            })
    }

    /// Hält eine Regel fest, wie sie zum Zeitpunkt der Entscheidung aussah.
    fn write_rule(&self, id: &str, yaml: &str, at: i64) -> Result<(), RecorderError> {
        self.conn
            .execute(
                "INSERT INTO rules_snapshot (id, yaml, first_seen, deleted_at) \
                 VALUES (?1, ?2, ?3, NULL) ON CONFLICT(id) DO UPDATE SET yaml = excluded.yaml, \
                 deleted_at = NULL",
                rusqlite::params![id, yaml, at],
            )
            .map(|_rows| ())
            .map_err(|err| storage_failed(format!("could not snapshot the rule {id} ({err})")))
    }

    /// Schreibt die Zusammenfassung eines Sandbox-Laufs.
    ///
    /// Ein zweiter Lauf derselben Sandbox-Kennung überschreibt die Zeile: Es
    /// gibt genau eine Zusammenfassung je Lauf, und die letzte ist die
    /// vollständige.
    fn write_session_summary(
        &self,
        session: SessionId,
        sandbox: SandboxId,
        at: i64,
        json: &str,
    ) -> Result<(), RecorderError> {
        self.conn
            .execute(
                "INSERT INTO session_summaries (sandbox_id, session_id, created, json) \
                 VALUES (?1, ?2, ?3, ?4) ON CONFLICT(sandbox_id) DO UPDATE SET \
                 session_id = excluded.session_id, created = excluded.created, \
                 json = excluded.json",
                rusqlite::params![
                    sandbox.to_string(),
                    session.to_string(),
                    at,
                    json.as_bytes()
                ],
            )
            .map(|_rows| ())
            .map_err(|err| {
                storage_failed(format!(
                    "could not store the session summary of sandbox {sandbox} ({err})"
                ))
            })
    }

    /// Vermerkt, dass es eine Regel nicht mehr gibt.
    fn write_rule_deleted(&self, id: &str, at: i64) -> Result<(), RecorderError> {
        self.conn
            .execute(
                "UPDATE rules_snapshot SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL",
                rusqlite::params![id, at],
            )
            .map(|_rows| ())
            .map_err(|err| {
                storage_failed(format!("could not mark the rule {id} as deleted ({err})"))
            })
    }

    /// Erhebt die Statistiken, aus denen der Abfrageplaner seine Wege wählt.
    ///
    /// Das gebündelte `SQLite` ist mit `SQLITE_ENABLE_STAT4` übersetzt, also
    /// hält `ANALYZE` nicht nur die Zeilenzahl je Index fest, sondern Stichproben
    /// über die Werte. Erst damit unterscheidet der Planer beim Filter `host:`
    /// den häufigen Host (dann lohnt der Lauf über `flows_ts` mit frühem
    /// Abbruch am `LIMIT`) vom seltenen (dann lohnt der Bereich über
    /// `flows_host_rev`). Ohne Statistiken rät er, und einer der beiden Fälle
    /// wird um ein Vielfaches langsamer.
    ///
    /// Läuft nach jedem Aufräumlauf, also üblicherweise einmal am Tag.
    fn analyze(&mut self) -> Result<(), RecorderError> {
        self.conn
            .execute_batch("ANALYZE;")
            .map_err(|err| storage_failed(format!("could not analyze the recorder ({err})")))
    }

    /// Löscht alles, was vor `before` liegt, und die Blobs, auf die danach
    /// niemand mehr zeigt.
    ///
    /// Gelöscht wird genau das: Flows mit `ts < before` samt ihren Nachrichten
    /// und Funden, Sitzungen, die danach leer und beendet sind, und
    /// Regel-Schnappschüsse, die vor `before` gelöscht wurden. Ein Blob wird
    /// erst entfernt, nachdem die Transaktion committet ist und eine erneute
    /// Abfrage zeigt, dass keine Zeile mehr auf ihn zeigt.
    fn purge(&mut self, before: i64) -> Result<PurgeReport, RecorderError> {
        let mut report = PurgeReport::default();
        let now = Instant::now();
        self.reserved
            .retain(|_sha256, at| now.duration_since(*at) < RESERVATION_GRACE);

        self.conn
            .execute_batch(
                "BEGIN;\nCREATE TEMP TABLE IF NOT EXISTS purge_ids (id TEXT PRIMARY KEY);\n\
                 DELETE FROM purge_ids;",
            )
            .map_err(|err| storage_failed(format!("could not start the retention pass ({err})")))?;

        let result = self.purge_inside(before, &mut report);
        match result {
            Ok(candidates) => {
                self.conn.execute_batch("COMMIT;").map_err(|err| {
                    storage_failed(format!("could not commit the retention pass ({err})"))
                })?;
                report.blobs = self.drop_unreferenced(&candidates);
                Ok(report)
            }
            Err(err) => {
                let _ignored = self.conn.execute_batch("ROLLBACK;");
                Err(err)
            }
        }
    }

    /// Der Teil des Aufräumens, der in der Transaktion läuft.
    fn purge_inside(
        &self,
        before: i64,
        report: &mut PurgeReport,
    ) -> Result<Vec<[u8; 32]>, RecorderError> {
        self.conn
            .execute(
                "INSERT INTO purge_ids (id) SELECT id FROM flows WHERE ts < ?1",
                rusqlite::params![before],
            )
            .map_err(|err| storage_failed(format!("could not select the old flows ({err})")))?;

        let candidates = {
            let mut statement = self
                .conn
                .prepare(
                    "SELECT DISTINCT blob_sha256 FROM messages WHERE blob_sha256 IS NOT NULL \
                     AND flow_id IN (SELECT id FROM purge_ids)",
                )
                .map_err(|err| {
                    storage_failed(format!("could not list the blobs of old flows ({err})"))
                })?;
            let rows = statement
                .query_map([], |row| row.get::<_, Vec<u8>>(0))
                .map_err(|err| {
                    storage_failed(format!("could not read the blobs of old flows ({err})"))
                })?;
            let mut out = Vec::new();
            for row in rows {
                let bytes = row.map_err(|err| {
                    storage_failed(format!("could not read a blob reference ({err})"))
                })?;
                if let Ok(sha256) = <[u8; 32]>::try_from(bytes.as_slice()) {
                    out.push(sha256);
                }
            }
            out
        };

        report.findings = self.delete(
            "DELETE FROM findings WHERE flow_id IN (SELECT id FROM purge_ids)",
            rusqlite::params![],
        )?;
        report.messages = self.delete(
            "DELETE FROM messages WHERE flow_id IN (SELECT id FROM purge_ids)",
            rusqlite::params![],
        )?;
        report.flows = self.delete(
            "DELETE FROM flows WHERE id IN (SELECT id FROM purge_ids)",
            rusqlite::params![],
        )?;
        report.sessions = self.delete(
            "DELETE FROM sessions WHERE ended_at IS NOT NULL AND ended_at < ?1 \
             AND id NOT IN (SELECT session_id FROM flows)",
            rusqlite::params![before],
        )?;
        self.delete(
            "DELETE FROM rules_snapshot WHERE deleted_at IS NOT NULL AND deleted_at < ?1",
            rusqlite::params![before],
        )?;
        self.conn
            .execute_batch("DELETE FROM purge_ids;")
            .map_err(|err| storage_failed(format!("could not clear the retention list ({err})")))?;
        Ok(candidates)
    }

    /// Führt ein `DELETE` aus und zählt die Zeilen.
    fn delete(&self, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<u64, RecorderError> {
        self.conn
            .execute(sql, params)
            .map(|rows| u64::try_from(rows).unwrap_or(0))
            .map_err(|err| storage_failed(format!("could not delete old rows ({err}): {sql}")))
    }

    /// Entfernt die Blobs, auf die nach dem Aufräumen niemand mehr zeigt.
    ///
    /// Ein angemeldeter Blob bleibt liegen, auch wenn gerade keine Zeile auf
    /// ihn zeigt: seine Zeile ist unterwegs. Ohne diese Ausnahme gäbe es ein
    /// schmales Fenster, in dem ein Aufräumlauf den Body einer Anfrage löschte,
    /// die denselben Inhalt noch einmal schickt.
    fn drop_unreferenced(&self, candidates: &[[u8; 32]]) -> u64 {
        let mut removed = 0;
        for sha256 in candidates {
            if self.reserved.contains_key(sha256) {
                continue;
            }
            let still_used: Result<i64, _> = self.conn.query_row(
                "SELECT COUNT(*) FROM messages WHERE blob_sha256 = ?1",
                rusqlite::params![sha256.as_slice()],
                |row| row.get(0),
            );
            match still_used {
                Ok(0) => match self.blobs.remove(sha256) {
                    Ok(true) => removed += 1,
                    Ok(false) => {}
                    Err(err) => self.report(err),
                },
                Ok(_) => {}
                Err(err) => self.report(storage_failed(format!(
                    "could not check whether a blob is still referenced ({err})"
                ))),
            }
        }
        removed
    }
}

/// Der Wert für `flows.upgrade`.
fn upgrade_of(headers: &HeaderMap, scheme: Scheme) -> Option<String> {
    if matches!(scheme, Scheme::Ws | Scheme::Wss) {
        return Some("websocket".to_owned());
    }
    let value = headers.get("upgrade")?.to_str().ok()?;
    if value.eq_ignore_ascii_case("websocket") {
        Some("websocket".to_owned())
    } else {
        None
    }
}

/// Die Regel hinter einer Entscheidung, als Text.
fn rule_of(source: DecisionSource, reason: Option<BlockReason>) -> Option<String> {
    if let DecisionSource::Rule(id) = source {
        return Some(id.to_string());
    }
    match reason {
        Some(BlockReason::Rule(id)) => Some(id.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::http::HeaderValue;
    use humanitl_core::{BlockReason, DecisionSource, HeaderMap, RuleId, Scheme};

    use super::{rule_of, upgrade_of};

    #[test]
    fn a_websocket_upgrade_is_recognised_from_header_and_scheme() {
        let mut headers = HeaderMap::new();
        assert_eq!(upgrade_of(&headers, Scheme::Https), None);
        headers.insert("upgrade", HeaderValue::from_static("WebSocket"));
        assert_eq!(
            upgrade_of(&headers, Scheme::Https),
            Some("websocket".to_owned())
        );
        assert_eq!(
            upgrade_of(&HeaderMap::new(), Scheme::Wss),
            Some("websocket".to_owned())
        );
    }

    #[test]
    fn the_rule_comes_from_the_source_or_from_the_block_reason() {
        let id = RuleId::new();
        assert_eq!(
            rule_of(DecisionSource::Rule(id), None),
            Some(id.to_string())
        );
        assert_eq!(
            rule_of(DecisionSource::User, Some(BlockReason::Rule(id))),
            Some(id.to_string())
        );
        assert_eq!(rule_of(DecisionSource::User, Some(BlockReason::User)), None);
    }
}
