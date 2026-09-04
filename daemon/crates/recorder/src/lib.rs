//! Aufzeichnung von Flows in `SQLite` samt Blob-Speicher.
//!
//! Siehe `docs/ARCHITECTURE.md` für die Schichtung und `backlog/CONVENTIONS.md`
//! Abschnitt 3.1 für die erlaubten Abhängigkeiten dieser Crate.
//!
//! Der Recorder setzt ADR-008 um: alles wird aufgezeichnet, und zwar dauerhaft.
//! Die Oberfläche hält nie mehr als eine Seite im Speicher; History, Export und
//! `ListFlows` lesen ausschließlich hier.
//!
//! # Aufbau
//!
//! ```text
//! FlowEvent ──apply──┐
//! Bodies ──store_message──┤   mpsc   ┌──────────────┐   SQLite (WAL)
//! Antwort ──ResponseSink──┼─────────>│ Writer-Thread│──> humanitl.db
//! Funde ──store_findings──┘          └──────────────┘    blobs/<xx>/<sha256>
//!
//! ListFlows / GetFlow ──spawn_blocking──> ReadPool (nur lesende Verbindungen)
//! ```
//!
//! Geschrieben wird in genau einem Thread, weil `rusqlite::Connection` `!Sync`
//! ist; gelesen wird nebenläufig auf eigenen Verbindungen. Im WAL-Modus stören
//! sich beide nicht (`https://www.sqlite.org/wal.html`).
//!
//! # Was hier nicht steht
//!
//! Bodies stehen nie in einem Logeintrag und nie in einem `Diagnostic`. Ein
//! Fehlertext nennt Flow-Id, Spalte und Pfad, nie Inhalt. Die Datenbankdateien
//! sind `0600`, die Verzeichnisse `0700`.
//!
//! # Beispiel
//!
//! ```no_run
//! use std::path::Path;
//! use humanitl_recorder::{Recorder, RecorderSettings};
//!
//! # async fn demo() -> Result<(), Box<dyn std::error::Error>> {
//! let recorder = Recorder::open(
//!     Path::new("/tmp/humanitl.db"),
//!     Path::new("/tmp/blobs"),
//!     RecorderSettings::default(),
//! )?;
//! let page = recorder.list_flows(&"host:github.com".parse()?).await?;
//! println!("{} Zeilen", page.rows.len());
//! # Ok(())
//! # }
//! ```
#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod blob;
mod error;
mod filter;
mod hostkey;
mod message;
mod query;
mod schema;
mod settings;
mod sink;
mod types;
mod writer;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;
use std::time::{Duration, SystemTime};

use bytes::Bytes;
use humanitl_core::{
    BodyRef, Diagnostic, Finding, FlowEvent, FlowId, HeaderMap, RuleId, SessionId,
};
use tokio::sync::{broadcast, oneshot};

pub use crate::blob::{BlobStore, ORPHAN_GRACE};
pub use crate::error::RecorderError;
pub use crate::filter::KEYS as FILTER_KEYS;
pub use crate::hostkey::{host_key, suffix_range};
pub use crate::query::ReadPool;
pub use crate::schema::{MIGRATIONS, Migration, latest_version};
pub use crate::settings::RecorderSettings;
pub use crate::sink::ResponseSink;
pub use crate::types::{
    COUNT_CEILING, Cursor, CursorKey, DEFAULT_LIMIT, Dir, FindingRecord, FlowDetail, FlowPage,
    FlowQuery, FlowSummary, MAX_LIMIT, MessageRecord, PurgeReport, SessionMeta, SortKey, millis,
};
pub use crate::writer::{BATCH_COMMANDS, BATCH_INTERVAL, RESERVATION_GRACE};

use crate::error::storage_failed;
use crate::writer::{MessageWrite, Writer, WriterCmd, WriterHandle};

/// So viele Befunde warten höchstens im Strom, bevor der langsamste Zuhörer
/// Ereignisse verliert.
const DIAGNOSTIC_BUFFER: usize = 256;

/// Ein Tag in Sekunden, für die Aufbewahrungsfrist.
const DAY_SECS: u64 = 24 * 60 * 60;

/// Das Handle auf die Aufzeichnung.
///
/// `Clone + Send + Sync`; jede Kopie schreibt in denselben Thread und liest aus
/// demselben Vorrat an Verbindungen. Der Schreib-Thread endet, wenn die letzte
/// Kopie fällt.
#[derive(Clone)]
pub struct Recorder {
    writer: Arc<WriterHandle>,
    read: Arc<ReadPool>,
    blobs: Arc<BlobStore>,
    settings: RecorderSettings,
    diagnostics: broadcast::Sender<Diagnostic>,
    db: Arc<PathBuf>,
}

impl core::fmt::Debug for Recorder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Recorder")
            .field("db", &self.db)
            .field("blobs", &self.blobs.root())
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl Recorder {
    /// Öffnet die Aufzeichnung, migriert das Schema und startet den Schreiber.
    ///
    /// Legt Datenverzeichnis (`0700`), Datenbank (`0600`) und Blob-Speicher
    /// (`0700`) an, falls sie fehlen, räumt liegengebliebene Temp-Dateien weg
    /// und entfernt Blobs, auf die niemand mehr zeigt und die älter sind als
    /// [`ORPHAN_GRACE`]. Damit übersteht die Aufzeichnung einen Absturz
    /// zwischen dem Schreiben eines Blobs und dem seiner Zeile.
    ///
    /// # Errors
    ///
    /// Ein [`Diagnostic`] mit `RECORDER_001`, wenn ein Verzeichnis, die
    /// Datenbank oder eine Migration nicht durchgeht, oder mit `RECORDER_004`,
    /// wenn der Blob-Speicher nicht benutzbar ist.
    pub fn open(db: &Path, blobs: &Path, settings: RecorderSettings) -> Result<Self, Diagnostic> {
        Self::open_inner(db, blobs, settings).map_err(RecorderError::into_diagnostic)
    }

    /// Der Rumpf von [`Recorder::open`], damit jeder Fehler denselben Weg geht.
    fn open_inner(
        db: &Path,
        blobs: &Path,
        settings: RecorderSettings,
    ) -> Result<Self, RecorderError> {
        let settings = settings.normalized();
        let conn = schema::open_write(db)?;
        schema::migrate(&conn, db)?;
        // Die Migration ist die erste Transaktion; erst dadurch entstehen
        // `-wal` und `-shm`, und zwar mit der Standardmaske des Prozesses. Sie
        // tragen den frischesten Teil der Aufzeichnung, also werden sie hier
        // auf `0600` gesetzt und nicht erst beim nächsten Start.
        schema::restrict(db)?;
        let backfilled = schema::backfill_host_rev(&conn)?;
        if backfilled > 0 {
            tracing::info!(rows = backfilled, "recorder backfilled host_rev");
        }

        let store = Arc::new(BlobStore::open(blobs)?);
        let removed_temp = store.sweep_temp();
        let referenced = query::referenced_blobs(&conn)?;
        let removed_orphans = store.sweep_orphans(&referenced, SystemTime::now());
        if removed_temp > 0 || removed_orphans > 0 {
            tracing::info!(
                temp = removed_temp,
                orphans = removed_orphans,
                "recorder swept leftover blobs"
            );
        }

        let (diagnostics, _idle) = broadcast::channel(DIAGNOSTIC_BUFFER);
        let (tx, rx) = mpsc::channel::<WriterCmd>();
        let writer = Writer::new(conn, db, Arc::clone(&store), diagnostics.clone());
        let join = std::thread::Builder::new()
            .name("humanitl-recorder".to_owned())
            .spawn(move || writer.run(&rx))
            .map_err(|err| {
                storage_failed(format!("could not start the recorder thread ({err})"))
            })?;

        Ok(Self {
            writer: Arc::new(WriterHandle::new(tx, join)),
            read: Arc::new(ReadPool::new(db)),
            blobs: store,
            settings,
            diagnostics,
            db: Arc::new(db.to_path_buf()),
        })
    }

    /// Die Grenzen, nach denen diese Aufzeichnung arbeitet.
    #[must_use]
    pub const fn settings(&self) -> RecorderSettings {
        self.settings
    }

    /// Der Pfad der Datenbank.
    #[must_use]
    pub fn database_path(&self) -> &Path {
        &self.db
    }

    /// Der Blob-Speicher, für Prüfungen und Werkzeuge.
    #[must_use]
    pub fn blobs(&self) -> &BlobStore {
        &self.blobs
    }

    /// Der Strom der Befunde des Schreib-Threads.
    ///
    /// Der Daemon hängt ihn an den Ereignisstrom (`FlowEvent::Diagnostic`),
    /// damit eine Lücke in der Aufzeichnung an derselben Stelle sichtbar wird
    /// wie der Flow, um den es geht.
    #[must_use]
    pub fn diagnostics(&self) -> broadcast::Receiver<Diagnostic> {
        self.diagnostics.subscribe()
    }

    /// Trägt eine Sitzung ein und macht sie zur laufenden.
    ///
    /// Muss vor dem ersten [`Recorder::apply`] laufen: eine Flow-Zeile braucht
    /// ihre Sitzung, und `flows.session_id` ist ein Fremdschlüssel.
    pub fn start_session(&self, meta: &SessionMeta) {
        self.send(WriterCmd::StartSession(Box::new(meta.clone())));
    }

    /// Schreibt das Ende einer Sitzung.
    pub fn end_session(&self, id: SessionId) {
        self.send(WriterCmd::EndSession {
            id,
            at: millis(SystemTime::now()),
        });
    }

    /// Schreibt fort, was ein Ereignis über den Flow sagt.
    ///
    /// Aufzurufen für jedes Ereignis des Proxys, in der Reihenfolge, in der es
    /// veröffentlicht wird. `ResponseChunk`, `Lagged` und `Diagnostic` ändern
    /// nichts; die Größe der Antwort kommt aus dem [`ResponseSink`].
    pub fn apply(&self, event: &FlowEvent) {
        self.send(WriterCmd::Event(Box::new(event.clone())));
    }

    /// Zeichnet eine vollständig gepufferte Nachricht auf.
    ///
    /// Bodies bis `recorder.inline_max_bytes` stehen in der Datenbank, größere
    /// als Datei im Blob-Speicher. Ein Body über
    /// `limits.recorder_max_body_bytes` wird gekürzt gespeichert und in
    /// `messages.truncated` vermerkt; `messages.size` trägt weiterhin die
    /// volle Länge.
    ///
    /// Der zurückgegebene [`BodyRef`] trägt die Prüfsumme über die
    /// gespeicherten Bytes: nur so findet [`Recorder::read_body`] wieder, was
    /// wirklich abgelegt wurde.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`], wenn der Body nicht in den Blob-Speicher
    /// geschrieben werden konnte.
    pub async fn store_message(
        &self,
        flow: FlowId,
        dir: Dir,
        headers: &HeaderMap,
        body: Bytes,
    ) -> Result<BodyRef, RecorderError> {
        let size = u64::try_from(body.len()).unwrap_or(u64::MAX);
        let cap = usize::try_from(self.settings.max_body_bytes).unwrap_or(usize::MAX);
        let truncated = body.len() > cap;
        let stored = if truncated { body.slice(..cap) } else { body };
        let sha256 = humanitl_core::http::sha256(&stored);

        let inline =
            u64::try_from(stored.len()).unwrap_or(u64::MAX) <= self.settings.inline_max_bytes;
        if !inline {
            let blobs = Arc::clone(&self.blobs);
            let data = stored.clone();
            tokio::task::spawn_blocking(move || blobs.put(&sha256, &data))
                .await
                .map_err(|err| {
                    storage_failed(format!("the blob writer did not finish ({err})"))
                })??;
        }

        let content_type = message::content_type_of(headers);
        self.send(WriterCmd::Message(Box::new(MessageWrite {
            flow,
            dir,
            headers_json: message::encode_headers(headers),
            content_type: content_type.clone(),
            content_encoding: message::content_encoding_of(headers),
            inline: if inline { Some(stored.clone()) } else { None },
            blob: if inline { None } else { Some(sha256) },
            size,
            truncated,
            status: None,
        })));

        Ok(BodyRef {
            sha256,
            size,
            inline: if inline { Some(stored) } else { None },
            content_type,
            truncated,
        })
    }

    /// Beginnt den Mitschnitt einer streamenden Antwort.
    ///
    /// Der Aufrufer schiebt jedes Stück durch [`ResponseSink::chunk`] und
    /// schließt mit [`ResponseSink::finish`] oder, wenn der Client aufgab, mit
    /// [`ResponseSink::abort`].
    #[must_use]
    pub fn begin_response(&self, flow: FlowId, headers: &HeaderMap) -> ResponseSink {
        ResponseSink::new(
            flow,
            Arc::clone(&self.writer),
            Arc::clone(&self.blobs),
            self.diagnostics.clone(),
            self.settings,
            headers,
        )
    }

    /// Zeichnet die Funde eines Flows auf und schreibt ihre Anzahl fort.
    pub fn store_findings(&self, flow: FlowId, findings: &[Finding]) {
        self.send(WriterCmd::Findings {
            flow,
            findings: findings.to_vec(),
        });
    }

    /// Trägt nach, woran ein Flow gescheitert ist (`flows.error`).
    ///
    /// Für den einen Fall, den kein [`FlowEvent`] trägt: Der Client in der
    /// Sandbox bricht den TLS-Handschlag zum Proxy ab, es gibt keine Anfrage
    /// und niemanden, der entschieden hätte, und die History soll den Versuch
    /// trotzdem zeigen (HUM-045). Der Weg nach draußen schreibt die Spalte
    /// dagegen von selbst aus [`FlowEvent::Failed`].
    ///
    /// `error` ist ein kurzer, fester Bezeichner in `snake_case`, kein Satz:
    /// `tls_handshake_failed`, `upstream_tls`, `upstream_dns`. Der Satz für den
    /// Menschen steht im [`Diagnostic`] am selben Flow. Ein schon gesetzter
    /// Grund bleibt stehen.
    ///
    /// Aufzurufen erst, nachdem [`FlowEvent::Received`] durch
    /// [`Recorder::apply`] gegangen ist: vorher gibt es die Zeile nicht, die
    /// hier fortgeschrieben wird. Beide Wege gehen durch denselben Kanal, also
    /// genügt die Reihenfolge der Aufrufe.
    pub fn set_flow_error(&self, flow: FlowId, error: &str) {
        self.send(WriterCmd::FlowError {
            flow,
            error: error.to_owned(),
        });
    }

    /// Trägt Apex und Katalog-Kennung eines Flows nach.
    ///
    /// Beides kennt der Recorder nicht selbst: die Public Suffix List und der
    /// Domain-Katalog liegen in `humanitl-catalog`, und diese Crate hängt nur
    /// von `humanitl-core` ab.
    pub fn set_domain(&self, flow: FlowId, apex: Option<String>, catalog_id: Option<String>) {
        self.send(WriterCmd::Domain {
            flow,
            apex,
            catalog_id,
        });
    }

    /// Hält eine Regel fest, wie sie zum Zeitpunkt der Entscheidung aussah.
    ///
    /// `rules.yaml` bleibt die Quelle der Wahrheit; der Schnappschuss sorgt
    /// dafür, dass die History eine gelöschte Regel noch anzeigen kann. Die
    /// Regel kommt als `YAML`, weil ihr Serialisierer in `humanitl-rules`
    /// wohnt und diese Crate ihn nicht kennen darf.
    pub fn snapshot_rule(&self, id: RuleId, yaml: &str) {
        self.send(WriterCmd::Rule {
            id: id.to_string(),
            yaml: yaml.to_owned(),
            at: millis(SystemTime::now()),
        });
    }

    /// Vermerkt, dass es eine Regel nicht mehr gibt.
    pub fn forget_rule(&self, id: RuleId) {
        self.send(WriterCmd::RuleDeleted {
            id: id.to_string(),
            at: millis(SystemTime::now()),
        });
    }

    /// Beantwortet eine Anfrage an die Flow-Liste.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Filter`] mit `RECORDER_002` bei einem unlesbaren
    /// Filter, [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn list_flows(&self, query: &FlowQuery) -> Result<FlowPage, RecorderError> {
        let read = Arc::clone(&self.read);
        let query = query.clone();
        let now = millis(SystemTime::now());
        tokio::task::spawn_blocking(move || query::list_flows(&read, &query, now))
            .await
            .map_err(|err| storage_failed(format!("the flow list did not finish ({err})")))?
    }

    /// Liefert einen Flow mit Nachrichten und Funden.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn get_flow(&self, id: FlowId) -> Result<Option<FlowDetail>, RecorderError> {
        let read = Arc::clone(&self.read);
        tokio::task::spawn_blocking(move || query::get_flow(&read, id))
            .await
            .map_err(|err| storage_failed(format!("the flow lookup did not finish ({err})")))?
    }

    /// Liest einen Body.
    ///
    /// Steht er inline im Verweis, kommt er von dort; sonst aus dem
    /// Blob-Speicher.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Blob`], wenn die Datei fehlt oder nicht lesbar ist.
    pub async fn read_body(&self, body: &BodyRef) -> Result<Bytes, RecorderError> {
        if let Some(inline) = &body.inline {
            return Ok(inline.clone());
        }
        if body.size == 0 {
            return Ok(Bytes::new());
        }
        let blobs = Arc::clone(&self.blobs);
        let sha256 = body.sha256;
        tokio::task::spawn_blocking(move || blobs.read(&sha256))
            .await
            .map_err(|err| storage_failed(format!("the blob read did not finish ({err})")))?
    }

    /// Löscht alles, was älter ist als die Aufbewahrungsfrist.
    ///
    /// Gelöscht wird genau das: Flows mit `ts` vor der Grenze samt ihren
    /// Nachrichten und Funden, beendete Sitzungen ohne Flows und
    /// Regel-Schnappschüsse, die vorher gelöscht wurden. Ein Blob fällt erst,
    /// wenn keine Zeile mehr auf ihn zeigt.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn purge_expired(&self, now: SystemTime) -> Result<PurgeReport, RecorderError> {
        let horizon = now
            .checked_sub(Duration::from_secs(
                u64::from(self.settings.retention_days) * DAY_SECS,
            ))
            .unwrap_or(SystemTime::UNIX_EPOCH);
        self.purge_before(horizon).await
    }

    /// Löscht alles, was vor diesem Zeitpunkt liegt.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn purge_before(&self, horizon: SystemTime) -> Result<PurgeReport, RecorderError> {
        let (reply, answer) = oneshot::channel();
        self.send(WriterCmd::Purge {
            before: millis(horizon),
            reply,
        });
        answer
            .await
            .map_err(|_closed| storage_failed("the recorder thread ended before it could purge"))?
    }

    /// Der Journal-Modus der Datenbank, für den Selbsttest beim Start.
    ///
    /// Muss `wal` sein: ohne WAL blockiert jeder Schreibvorgang die Leser
    /// (`https://www.sqlite.org/wal.html`).
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`], wenn sich das `PRAGMA` nicht lesen lässt.
    pub async fn journal_mode(&self) -> Result<String, RecorderError> {
        let read = Arc::clone(&self.read);
        tokio::task::spawn_blocking(move || read.with(schema::journal_mode))
            .await
            .map_err(|err| {
                storage_failed(format!("the journal mode read did not finish ({err})"))
            })?
    }

    /// Erhebt die Statistiken des Abfrageplaners neu (`ANALYZE`).
    ///
    /// Der Daemon braucht das nicht selbst aufzurufen: [`Recorder::purge_expired`]
    /// tut es am Ende jedes Aufräumlaufs. Die Methode steht für den ersten Start
    /// nach einer Migration und für Tests, die sofort messen wollen.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`], wenn `ANALYZE` scheitert.
    pub async fn analyze(&self) -> Result<(), RecorderError> {
        let (reply, answer) = oneshot::channel();
        self.send(WriterCmd::Analyze(reply));
        answer.await.map_err(|_closed| {
            storage_failed("the recorder thread ended before it could analyze")
        })?
    }

    /// Wartet, bis alles Geschickte in der Datenbank steht.
    ///
    /// Für den geordneten Abschied des Daemons und für Tests, die danach lesen.
    pub async fn flush(&self) {
        let (reply, answer) = oneshot::channel();
        self.send(WriterCmd::Flush(reply));
        let _ignored = answer.await;
    }

    /// Schickt ein Kommando; ein toter Schreiber ist ein Befund, kein Panic.
    fn send(&self, cmd: WriterCmd) {
        if let Err(err) = self.writer.send(cmd) {
            let diagnostic = err.into_diagnostic();
            tracing::error!(code = diagnostic.code.as_str(), why = %diagnostic.why, "recorder");
            let _ignored = self.diagnostics.send(diagnostic);
        }
    }
}

impl core::str::FromStr for FlowQuery {
    type Err = core::convert::Infallible;

    /// Eine Anfrage aus einem Filterausdruck, sonst Vorgaben.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self::new(s))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::Recorder;

    const fn assert_send_sync<T: Send + Sync + Clone>() {}

    #[test]
    fn the_handle_is_clone_send_and_sync() {
        assert_send_sync::<Recorder>();
    }
}
