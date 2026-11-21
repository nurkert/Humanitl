//! Das Projektregister (HUM-532, ADR-0024 Punkt 1).
//!
//! Ein Projekt ist eine Zeile in `projects` mit stabiler [`ProjectId`]. Ein
//! Start legt die Zeile an oder zählt sie hoch, und im selben Schritt bekommt
//! die Sitzung in `sessions` die `project_id` (`register_start`). Alles, was
//! je Projekt auswählt, geht über diese Kennung (`sessions_of_project`),
//! nie über `work_dir`: Am selben Pfad kann nacheinander mehr als ein Ordner
//! gelegen haben.
//!
//! # Wiedererkennen
//!
//! Der Pfad ist kanonisch (das schreibt der Aufrufer), `dev` und `ino` stammen
//! vom ersten Start. Liegt am Pfad ein Ordner mit anderem Paar, ist es ein
//! anderes Projekt am alten Ort: Die alte Zeile bekommt `replaced_at` und
//! bleibt samt ihrer Wahl als Verlauf, und in derselben Transaktion entsteht
//! eine neue aktuelle Zeile mit neuer Kennung und ohne Wahl. Eine Zeile ohne
//! Paar (nachgefüllt aus Sitzungen von vor dem Register) übernimmt das Paar
//! des nächsten Starts.
//!
//! Unter macOS faltet APFS die Schreibweise eines Namens: `~/Proj` und
//! `~/proj` sind derselbe Ordner. Eine aktuelle Zeile, deren Pfad sich nur in
//! der Schreibweise unterscheidet und die dasselbe Paar trägt, ist dort
//! dasselbe Projekt ([`same_place`]).
//!
//! Die Projektwahl (`choice`) ist hier Text. Was sie tragen darf, entscheidet
//! `humanitl_config::ProjectChoice`; diese Crate kennt den Typ nicht
//! (`tools/deps-allow.toml`: nur `humanitl-core`).

use std::time::SystemTime;

use humanitl_core::{ProjectId, SessionId};
use rusqlite::{Connection, OptionalExtension as _, Row};

use crate::error::{RecorderError, storage_failed};
use crate::types::{at_millis, millis};

/// Die Spalten einer Zeile, in der Reihenfolge von [`row_to_project`].
const COLUMNS: &str = "id, path, dev, ino, display_name, first_seen, last_started, last_agent, \
                       runs, choice, choice_saved_at, replaced_at, version";

/// Eine Zeile aus `projects`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectRow {
    /// Die stabile Kennung.
    pub id: ProjectId,
    /// Der kanonische Pfad auf dem Host.
    pub path: String,
    /// Gerät des Ordners beim ersten Start; leer für eine nachgefüllte Zeile.
    pub dev: Option<u64>,
    /// Inode des Ordners beim ersten Start; leer für eine nachgefüllte Zeile.
    pub ino: Option<u64>,
    /// Der Ordnername zur Anzeige, ungesäubert.
    pub display_name: String,
    /// Wann das Projekt zuerst gesehen wurde.
    pub first_seen: SystemTime,
    /// Der letzte Start.
    pub last_started: Option<SystemTime>,
    /// Der Agent des letzten Starts.
    pub last_agent: Option<String>,
    /// Zahl der Starts.
    pub runs: u64,
    /// Die Projektwahl als JSON-Text.
    pub choice: Option<String>,
    /// Wann die Wahl zuletzt gespeichert wurde.
    pub choice_saved_at: Option<SystemTime>,
    /// Wann ein anderer Ordner am selben Pfad diese Zeile ersetzt hat; leer
    /// für die aktuelle.
    pub replaced_at: Option<SystemTime>,
    /// Steigt bei jeder Änderung der Zeile.
    pub version: u64,
}

/// Was ein Start über sein Projekt weiß.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectStart {
    /// Die Sitzung des Daemons, die den Start trägt.
    pub session: SessionId,
    /// Der kanonische Pfad.
    pub path: String,
    /// Gerät des Ordners.
    pub dev: u64,
    /// Inode des Ordners.
    pub ino: u64,
    /// Der Agent-Adapter des Starts.
    pub agent: String,
    /// Der Zeitpunkt des Starts.
    pub at: SystemTime,
}

/// Ein Ordner, für den die Projektwahl vor seinem ersten Start gespeichert
/// wird (HUM-538): der Erststart-Bildschirm speichert, bevor er startet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFolder {
    /// Der kanonische Pfad.
    pub path: String,
    /// Gerät des Ordners.
    pub dev: u64,
    /// Inode des Ordners.
    pub ino: u64,
}

/// Was `register_start` getan hat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registered {
    /// Die aktuelle Zeile nach dem Start.
    pub project: ProjectRow,
    /// Wahr, wenn die Zeile mit diesem Start entstand (neues Projekt oder
    /// ersetzter Ordner).
    pub created: bool,
    /// Die Zeile, die dieser Start ersetzt hat.
    pub replaced: Option<ProjectId>,
}

/// Ob zwei Pfade denselben Ort nennen: gleich geschrieben, unter macOS auch
/// in anderer Schreibweise (APFS faltet sie).
#[must_use]
pub fn same_place(stored: &str, wanted: &str) -> bool {
    if stored == wanted {
        return true;
    }
    cfg!(target_os = "macos") && stored.to_lowercase() == wanted.to_lowercase()
}

/// Die aktuelle Zeile an diesem Pfad, ohne den Ordner zu prüfen.
///
/// Unter macOS auch eine Zeile, deren Pfad sich nur in der Schreibweise
/// unterscheidet und die das Paar `identity` trägt; ohne Paar nur die
/// gleich geschriebene.
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn current_at(
    conn: &Connection,
    path: &str,
    identity: Option<(u64, u64)>,
) -> Result<Option<ProjectRow>, RecorderError> {
    let exact = conn
        .query_row(
            &format!("SELECT {COLUMNS} FROM projects WHERE path = ?1 AND replaced_at IS NULL"),
            [path],
            row_to_project,
        )
        .optional()
        .map_err(|err| {
            storage_failed(format!("could not look up the project at a path ({err})"))
        })?;
    if exact.is_some() || !cfg!(target_os = "macos") {
        return Ok(exact);
    }
    // Unter macOS: eine Zeile mit diesem Paar, oder eine nachgefüllte ohne
    // Paar, deren Pfad sich nur in der Schreibweise unterscheidet; der
    // nächste Start übernimmt dann das Paar.
    let (dev, ino) = identity.map_or((None, None), |(dev, ino)| {
        (Some(to_sql(dev)), Some(to_sql(ino)))
    });
    let candidates = select(
        conn,
        &format!(
            "SELECT {COLUMNS} FROM projects WHERE replaced_at IS NULL \
             AND ((dev = ?1 AND ino = ?2) OR dev IS NULL)"
        ),
        rusqlite::params![dev, ino],
    )?;
    Ok(pick_folded(candidates, path, identity, identity_of))
}

/// Unter den Kandidaten die Zeile, die denselben Ordner nennt: Der Pfad
/// unterscheidet sich höchstens in der Schreibweise, und eine Zeile mit Paar
/// trägt das Paar des Ordners. Eine nachgefüllte Zeile ohne Paar gilt nur,
/// wenn ihr gespeicherter Pfad heute denselben Ordner nennt (`stat` liefert
/// dasselbe Paar): Auf einem Volume, das die Schreibweise unterscheidet,
/// sind `/Proj` und `/proj` zwei Ordner, und das zweite erbte sonst Kennung
/// und Wahl des ersten.
#[must_use]
pub(crate) fn pick_folded(
    candidates: Vec<ProjectRow>,
    path: &str,
    identity: Option<(u64, u64)>,
    stat: impl Fn(&str) -> Option<(u64, u64)>,
) -> Option<ProjectRow> {
    let wanted = identity?;
    candidates.into_iter().find(|row| {
        same_place(&row.path, path)
            && match row.dev.zip(row.ino) {
                Some(known) => known == wanted,
                None => stat(&row.path) == Some(wanted),
            }
    })
}

/// Gerät und Inode des Ordners an einem Pfad, falls es ihn gibt.
fn identity_of(path: &str) -> Option<(u64, u64)> {
    use std::os::unix::fs::MetadataExt as _;
    std::fs::metadata(path)
        .ok()
        .filter(std::fs::Metadata::is_dir)
        .map(|meta| (meta.dev(), meta.ino()))
}

/// Eine Zeile an ihrer Kennung, aktuell oder ersetzt.
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn get(conn: &Connection, id: ProjectId) -> Result<Option<ProjectRow>, RecorderError> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM projects WHERE id = ?1"),
        [id.to_string()],
        row_to_project,
    )
    .optional()
    .map_err(|err| storage_failed(format!("could not read the project {id} ({err})")))
}

/// Eine Seite der aktuellen Zeilen, der jüngste Start zuerst; ein Projekt
/// ohne Start (nachgefüllt) steht nach seinem ersten Auftreten.
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn list(conn: &Connection, limit: u32, offset: u32) -> Result<Vec<ProjectRow>, RecorderError> {
    select(
        conn,
        &format!(
            "SELECT {COLUMNS} FROM projects WHERE replaced_at IS NULL \
             ORDER BY COALESCE(last_started, first_seen) DESC, id DESC LIMIT ?1 OFFSET ?2"
        ),
        rusqlite::params![i64::from(limit), i64::from(offset)],
    )
}

/// Jede Zeile, aktuell und ersetzt, der jüngste Start zuerst (für das
/// Aufräumen mit der Auswahl „alles", HUM-533).
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub(crate) fn all_rows(conn: &Connection) -> Result<Vec<ProjectRow>, RecorderError> {
    select(
        conn,
        &format!(
            "SELECT {COLUMNS} FROM projects \
             ORDER BY COALESCE(last_started, first_seen) DESC, id DESC"
        ),
        [],
    )
}

/// Die Sitzungen eines Projekts, die jüngste zuerst; ausgewählt über
/// `project_id`, nie über den Pfad.
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn sessions_of_project(
    conn: &Connection,
    id: ProjectId,
) -> Result<Vec<SessionId>, RecorderError> {
    let mut statement = conn
        .prepare("SELECT id FROM sessions WHERE project_id = ?1 ORDER BY started_at DESC, id DESC")
        .map_err(|err| storage_failed(format!("could not list the sessions of {id} ({err})")))?;
    let rows = statement
        .query_map([id.to_string()], |row| row.get::<_, String>(0))
        .map_err(|err| storage_failed(format!("could not read the sessions of {id} ({err})")))?;
    let mut out = Vec::new();
    for row in rows {
        let text =
            row.map_err(|err| storage_failed(format!("could not read a session ({err})")))?;
        out.push(text.parse().unwrap_or_else(|_err| SessionId::nil()));
    }
    Ok(out)
}

/// Trägt einen Start ein, in einer Transaktion: Projekt anlegen, ersetzen
/// oder hochzählen, und die Sitzung dem Projekt zuordnen.
///
/// Die Sitzung muss in `sessions` stehen; ihre Spalten `work_dir` und `agent`
/// nennen danach den Ordner und den Agenten dieses Starts.
///
/// # Errors
///
/// [`RecorderError::Storage`], wenn eine Anweisung scheitert; dann ist nichts
/// geschrieben.
pub fn register_start(
    conn: &Connection,
    start: &ProjectStart,
) -> Result<Registered, RecorderError> {
    in_transaction(conn, "register a project start", || {
        register_inside(conn, start)
    })
}

/// Speichert die Projektwahl einer aktuellen Zeile; `None` leert sie.
///
/// `None` als Antwort heißt: Es gibt keine aktuelle Zeile mit dieser
/// Kennung (unbekannt oder ersetzt), und nichts wurde geschrieben. Eine
/// ersetzte Zeile behält die Wahl, die sie hatte; sie ist Verlauf.
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn save_choice(
    conn: &Connection,
    id: ProjectId,
    choice: Option<&str>,
    at: SystemTime,
) -> Result<Option<ProjectRow>, RecorderError> {
    in_transaction(conn, "save a project choice", || {
        let changed = conn
            .execute(
                "UPDATE projects SET choice = ?2, choice_saved_at = ?3, version = version + 1 \
                 WHERE id = ?1 AND replaced_at IS NULL",
                rusqlite::params![id.to_string(), choice, millis(at)],
            )
            .map_err(|err| {
                storage_failed(format!(
                    "could not save the choice of the project {id} ({err})"
                ))
            })?;
        if changed == 0 {
            return Ok(None);
        }
        get(conn, id)
    })
}

/// Speichert die Projektwahl des Ordners `folder`, in einer Transaktion,
/// und trägt ihn ins Register ein, wenn es ihn noch nicht kennt (HUM-538).
///
/// Die Zeile entsteht ohne Start: `runs` 0, kein `last_started`, kein
/// Agent. Der erste Start findet sie dann an Pfad, Gerät und Inode, lädt
/// ihre Wahl und zählt hoch wie bei jedem bekannten Projekt. Liegt am Pfad
/// eine aktuelle Zeile mit anderem Paar, ist der Ordner ein anderes Projekt
/// am alten Ort, wie beim Start: Die alte Zeile wird Verlauf und behält ihre
/// Wahl, die neue bekommt diese.
///
/// # Errors
///
/// [`RecorderError::Storage`], wenn eine Anweisung scheitert; dann ist nichts
/// geschrieben.
pub fn save_folder_choice(
    conn: &Connection,
    folder: &ProjectFolder,
    choice: Option<&str>,
    at: SystemTime,
) -> Result<ProjectRow, RecorderError> {
    in_transaction(conn, "save the choice of a folder", || {
        let identity = (folder.dev, folder.ino);
        let id = match current_at(conn, &folder.path, Some(identity))? {
            Some(row) if row.dev.zip(row.ino).is_some_and(|known| known != identity) => {
                mark_replaced(conn, row.id, millis(at))?;
                insert_unstarted(conn, folder, at)?
            }
            Some(row) => {
                conn.execute(
                    "UPDATE projects SET dev = COALESCE(dev, ?2), ino = COALESCE(ino, ?3) \
                     WHERE id = ?1",
                    rusqlite::params![row.id.to_string(), to_sql(folder.dev), to_sql(folder.ino)],
                )
                .map_err(|err| {
                    storage_failed(format!("could not read the folder of {} ({err})", row.id))
                })?;
                row.id
            }
            None => insert_unstarted(conn, folder, at)?,
        };
        conn.execute(
            "UPDATE projects SET choice = ?2, choice_saved_at = ?3, version = version + 1 \
             WHERE id = ?1",
            rusqlite::params![id.to_string(), choice, millis(at)],
        )
        .map_err(|err| {
            storage_failed(format!(
                "could not save the choice of the project {id} ({err})"
            ))
        })?;
        get(conn, id)?
            .ok_or_else(|| storage_failed(format!("the project {id} is gone right after saving")))
    })
}

/// Markiert eine Zeile als ersetzt: Ein anderer Ordner liegt an ihrem Pfad.
fn mark_replaced(conn: &Connection, id: ProjectId, at: i64) -> Result<(), RecorderError> {
    conn.execute(
        "UPDATE projects SET replaced_at = ?2, version = version + 1 WHERE id = ?1",
        rusqlite::params![id.to_string(), at],
    )
    .map_err(|err| storage_failed(format!("could not mark the project {id} replaced ({err})")))?;
    Ok(())
}

/// Eine neue aktuelle Zeile ohne Start.
fn insert_unstarted(
    conn: &Connection,
    folder: &ProjectFolder,
    at: SystemTime,
) -> Result<ProjectId, RecorderError> {
    let id = ProjectId::new();
    conn.execute(
        "INSERT INTO projects (id, path, dev, ino, display_name, first_seen, runs) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
        rusqlite::params![
            id.to_string(),
            folder.path,
            to_sql(folder.dev),
            to_sql(folder.ino),
            folder_name(&folder.path),
            millis(at),
        ],
    )
    .map_err(|err| storage_failed(format!("could not register the project {id} ({err})")))?;
    Ok(id)
}

fn register_inside(conn: &Connection, start: &ProjectStart) -> Result<Registered, RecorderError> {
    let identity = (start.dev, start.ino);
    let at = millis(start.at);
    let current = current_at(conn, &start.path, Some(identity))?;
    let (id, created, replaced) = match current {
        Some(row) if row.dev.zip(row.ino).is_some_and(|known| known != identity) => {
            mark_replaced(conn, row.id, at)?;
            (insert(conn, start)?, true, Some(row.id))
        }
        Some(row) => {
            conn.execute(
                "UPDATE projects SET dev = COALESCE(dev, ?2), ino = COALESCE(ino, ?3), \
                 last_started = ?4, last_agent = ?5, runs = runs + 1, version = version + 1 \
                 WHERE id = ?1",
                rusqlite::params![
                    row.id.to_string(),
                    to_sql(start.dev),
                    to_sql(start.ino),
                    at,
                    start.agent
                ],
            )
            .map_err(|err| {
                storage_failed(format!("could not count the start of {} ({err})", row.id))
            })?;
            (row.id, false, None)
        }
        None => (insert(conn, start)?, true, None),
    };
    conn.execute(
        "UPDATE sessions SET project_id = ?2, work_dir = ?3, agent = ?4 WHERE id = ?1",
        rusqlite::params![
            start.session.to_string(),
            id.to_string(),
            start.path,
            start.agent
        ],
    )
    .map_err(|err| {
        storage_failed(format!(
            "could not assign the session {} to the project {id} ({err})",
            start.session
        ))
    })?;
    let project = get(conn, id)?
        .ok_or_else(|| storage_failed(format!("the project {id} is gone right after its start")))?;
    Ok(Registered {
        project,
        created,
        replaced,
    })
}

fn insert(conn: &Connection, start: &ProjectStart) -> Result<ProjectId, RecorderError> {
    let id = ProjectId::new();
    conn.execute(
        "INSERT INTO projects (id, path, dev, ino, display_name, first_seen, last_started, \
         last_agent, runs) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?7, 1)",
        rusqlite::params![
            id.to_string(),
            start.path,
            to_sql(start.dev),
            to_sql(start.ino),
            folder_name(&start.path),
            millis(start.at),
            start.agent,
        ],
    )
    .map_err(|err| storage_failed(format!("could not register the project {id} ({err})")))?;
    Ok(id)
}

/// Der letzte Teil des Pfades, sonst der Pfad selbst (`/`).
fn folder_name(path: &str) -> String {
    path.trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(path)
        .to_owned()
}

/// Führt `work` in einer eigenen Transaktion aus; scheitert es, bleibt nichts.
fn in_transaction<T>(
    conn: &Connection,
    what: &str,
    work: impl FnOnce() -> Result<T, RecorderError>,
) -> Result<T, RecorderError> {
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|err| storage_failed(format!("could not begin to {what} ({err})")))?;
    match work() {
        Ok(value) => {
            conn.execute_batch("COMMIT;").map_err(|err| {
                let _ignored = conn.execute_batch("ROLLBACK;");
                storage_failed(format!("could not commit to {what} ({err})"))
            })?;
            Ok(value)
        }
        Err(err) => {
            let _ignored = conn.execute_batch("ROLLBACK;");
            Err(err)
        }
    }
}

fn select<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<ProjectRow>, RecorderError> {
    let mut statement = conn
        .prepare(sql)
        .map_err(|err| storage_failed(format!("could not list the projects ({err})")))?;
    let rows = statement
        .query_map(params, row_to_project)
        .map_err(|err| storage_failed(format!("could not read the projects ({err})")))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| storage_failed(format!("could not read a project ({err})")))?);
    }
    Ok(out)
}

/// `dev` und `ino` sind `u64`; `SQLite` speichert `i64`. Das Bitmuster bleibt
/// erhalten, damit ein Vergleich nach dem Rücklesen gilt.
#[allow(clippy::cast_possible_wrap)]
const fn to_sql(value: u64) -> i64 {
    value as i64
}

#[allow(clippy::cast_sign_loss)]
const fn from_sql(value: i64) -> u64 {
    value as u64
}

/// Eine Zeile aus `projects`. Eine Kennung, die sich nicht lesen lässt, wird
/// zur Nil-Id, wie in `session_summaries`.
fn row_to_project(row: &Row<'_>) -> rusqlite::Result<ProjectRow> {
    let id: String = row.get(0)?;
    let runs: i64 = row.get(8)?;
    let version: i64 = row.get(12)?;
    Ok(ProjectRow {
        id: id.parse().unwrap_or_else(|_err| ProjectId::nil()),
        path: row.get(1)?,
        dev: row.get::<_, Option<i64>>(2)?.map(from_sql),
        ino: row.get::<_, Option<i64>>(3)?.map(from_sql),
        display_name: row.get(4)?,
        first_seen: at_millis(row.get(5)?),
        last_started: row.get::<_, Option<i64>>(6)?.map(at_millis),
        last_agent: row.get(7)?,
        runs: u64::try_from(runs).unwrap_or(0),
        choice: row.get(9)?,
        choice_saved_at: row.get::<_, Option<i64>>(10)?.map(at_millis),
        replaced_at: row.get::<_, Option<i64>>(11)?.map(at_millis),
        version: u64::try_from(version).unwrap_or(0),
    })
}

impl crate::Recorder {
    /// Trägt einen Start ins Projektregister ein und ordnet die Sitzung ihm
    /// zu, in einer Transaktion (`register_start`).
    ///
    /// Die Sitzung muss vorher mit [`crate::Recorder::start_session`]
    /// eingetragen sein; der Schreib-Thread schreibt die Kommandos in der
    /// Reihenfolge, in der sie kommen.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank oder wenn der
    /// Schreib-Thread nicht mehr da ist.
    pub async fn register_project_start(
        &self,
        start: ProjectStart,
    ) -> Result<Registered, RecorderError> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.send(crate::writer::WriterCmd::RegisterProject {
            start: Box::new(start),
            reply,
        });
        answer.await.map_err(|_closed| {
            storage_failed("the recorder thread ended before it could register the project")
        })?
    }

    /// Speichert die Projektwahl einer aktuellen Zeile (`save_choice`).
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn save_project_choice(
        &self,
        id: ProjectId,
        choice: Option<String>,
    ) -> Result<Option<ProjectRow>, RecorderError> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.send(crate::writer::WriterCmd::SaveProjectChoice {
            id,
            choice,
            at: SystemTime::now(),
            reply,
        });
        answer.await.map_err(|_closed| {
            storage_failed("the recorder thread ended before it could save the project choice")
        })?
    }

    /// Speichert die Projektwahl eines Ordners und trägt ihn ein, wenn das
    /// Register ihn nicht kennt (`save_folder_choice`, HUM-538).
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn save_folder_choice(
        &self,
        folder: ProjectFolder,
        choice: Option<String>,
    ) -> Result<ProjectRow, RecorderError> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.send(crate::writer::WriterCmd::SaveFolderChoice {
            folder: Box::new(folder),
            choice,
            at: SystemTime::now(),
            reply,
        });
        answer.await.map_err(|_closed| {
            storage_failed("the recorder thread ended before it could save the folder choice")
        })?
    }

    /// Eine Seite der aktuellen Projekte, der jüngste Start zuerst (`list`).
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn list_projects(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<ProjectRow>, RecorderError> {
        self.read_projects(move |conn| list(conn, limit, offset))
            .await
    }

    /// Ein Projekt an seiner Kennung, aktuell oder ersetzt (`get`).
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn get_project(&self, id: ProjectId) -> Result<Option<ProjectRow>, RecorderError> {
        self.read_projects(move |conn| get(conn, id)).await
    }

    /// Das aktuelle Projekt an einem kanonischen Pfad (`current_at`).
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn current_project_at(
        &self,
        path: String,
        identity: Option<(u64, u64)>,
    ) -> Result<Option<ProjectRow>, RecorderError> {
        self.read_projects(move |conn| current_at(conn, &path, identity))
            .await
    }

    /// Die Sitzungen eines Projekts (`sessions_of_project`).
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
    pub async fn sessions_of_project(
        &self,
        id: ProjectId,
    ) -> Result<Vec<SessionId>, RecorderError> {
        self.read_projects(move |conn| sessions_of_project(conn, id))
            .await
    }

    /// Liest auf einer Nur-Lese-Verbindung, abseits des Laufzeit-Threads.
    async fn read_projects<T, F>(&self, work: F) -> Result<T, RecorderError>
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, RecorderError> + Send + 'static,
    {
        let read = std::sync::Arc::clone(&self.read);
        tokio::task::spawn_blocking(move || read.with(work))
            .await
            .map_err(|err| storage_failed(format!("the project lookup did not finish ({err})")))?
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::ProjectId;

    use super::{ProjectRow, folder_name, pick_folded, same_place};

    #[test]
    fn the_folder_name_is_the_last_part() {
        assert_eq!(folder_name("/home/u/code/humanitl"), "humanitl");
        assert_eq!(folder_name("/home/u/code/humanitl/"), "humanitl");
        assert_eq!(folder_name("/"), "/");
    }

    fn row(path: &str, pair: Option<(u64, u64)>) -> ProjectRow {
        ProjectRow {
            id: ProjectId::new(),
            path: path.to_owned(),
            dev: pair.map(|(dev, _)| dev),
            ino: pair.map(|(_, ino)| ino),
            display_name: String::new(),
            first_seen: std::time::UNIX_EPOCH,
            last_started: None,
            last_agent: None,
            runs: 0,
            choice: None,
            choice_saved_at: None,
            replaced_at: None,
            version: 1,
        }
    }

    /// HUM-532, Codex: Eine nachgefüllte Zeile `/Proj` ohne Paar und ein
    /// anderer Ordner `/proj` auf einem Volume, das die Schreibweise
    /// unterscheidet, sind zwei Projekte; nur wenn `/Proj` heute derselbe
    /// Ordner ist, gilt die Zeile. Die Faltung selbst gilt nur unter macOS;
    /// unter Linux findet keiner der Fälle eine Zeile.
    ///
    /// Mutationsbeweis: Nähme `pick_folded` jede Zeile ohne Paar, bekäme der
    /// fremde Ordner die Zeile, und der Test wird unter macOS rot; der Fall
    /// mit gleichem Pfad prüft die Regel auch unter Linux.
    #[test]
    fn a_row_without_a_pair_matches_only_the_same_folder() {
        let stored = || vec![row("/Users/u/Proj", None)];
        let other_folder = |_: &str| Some((1, 2));
        let same_folder = |_: &str| Some((1, 9));
        assert_eq!(
            pick_folded(stored(), "/Users/u/proj", Some((1, 9)), other_folder),
            None
        );
        assert_eq!(
            pick_folded(stored(), "/Users/u/proj", Some((1, 9)), same_folder).is_some(),
            cfg!(target_os = "macos")
        );
        // Derselbe Pfad, dieselbe Regel, auf jedem System.
        assert_eq!(
            pick_folded(stored(), "/Users/u/Proj", Some((1, 9)), other_folder),
            None
        );
        assert!(pick_folded(stored(), "/Users/u/Proj", Some((1, 9)), same_folder).is_some());
        assert_eq!(
            pick_folded(stored(), "/Users/u/Proj", None, same_folder),
            None
        );
        // Eine Zeile mit Paar: nur dieses Paar.
        let paired = || vec![row("/Users/u/Proj", Some((1, 9)))];
        assert!(pick_folded(paired(), "/Users/u/Proj", Some((1, 9)), other_folder).is_some());
        assert_eq!(
            pick_folded(paired(), "/Users/u/Proj", Some((1, 8)), same_folder),
            None
        );
    }

    #[test]
    fn spelling_counts_only_on_macos() {
        assert!(same_place("/Users/u/Proj", "/Users/u/Proj"));
        assert_eq!(
            same_place("/Users/u/Proj", "/Users/u/proj"),
            cfg!(target_os = "macos")
        );
    }
}
