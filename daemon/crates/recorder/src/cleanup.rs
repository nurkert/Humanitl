//! Aufräumen und Vergessen (HUM-533, ADR-0024 Punkt 14).
//!
//! Gelöscht wird nur, was vorher als Plan einzeln aufgelistet und von einem
//! Menschen bestätigt wurde (Grenze 3 von Sprint 8). Der Plan hat einen
//! Digest; ausgeführt wird er mit [`execute`], das in **einer** Transaktion
//! den Plan mit derselben Auswahl neu berechnet, den Digest vergleicht und
//! nur bei Gleichheit löscht. Jede Änderung dazwischen (neue Sitzung, neuer
//! Fluss, eine andere Projektwahl, auch gleich lang, ein Start im Projekt)
//! ergibt einen anderen Digest und damit [`Executed::Stale`]: Nichts ist
//! gelöscht.
//!
//! # Was der Digest bindet
//!
//! Nicht die Anzeige (Zahlen und Größen blieben bei einer anderen, gleich
//! großen Fassung gleich), sondern Identität und Fassung jedes Eintrags: die
//! Zeilen-`id`s der Sitzungen und Flüsse, die Hashes der Blobs samt der
//! Angabe, ob sie fallen, die Kennung der wartenden Schicht, je Projekt
//! `id` und `version`, und den SHA-256 des Textes der Projektwahl. Dazu die
//! Auswahl selbst.
//!
//! # Arten
//!
//! - [`CleanupKind::RecordedFlows`]: Flüsse samt Nachrichten, Funden,
//!   Befunden und Bodies der Sitzungen eines Projekts. Die Zeilen der
//!   Sitzungen und ihre Zusammenfassungen bleiben (sie nennen Lauf und
//!   Ordner, keinen Verkehr). Ein Blob fällt nur, wenn nach dem Löschen keine
//!   Zeile mehr auf ihn zeigt; der Speicher ist inhaltsadressiert, und
//!   derselbe Body kann in einem anderen Projekt stehen.
//! - [`CleanupKind::WaitingChanges`]: die wartende Schicht einer
//!   Wegwerf-Sandbox. Sie liegt nicht in dieser Datenbank, sondern im
//!   Arbeitsspeicher des Daemons; der Aufrufer nennt sie in [`LiveState`],
//!   der Digest bindet ihre Kennung, und verworfen wird sie vom Aufrufer
//!   nach dem Commit. Nur mit ausdrücklicher Wahl
//!   ([`CleanupScope::include_waiting`]).
//! - [`CleanupKind::ProjectChoice`] und [`CleanupKind::RegisterEntry`]: die
//!   Projektwahl und die Zeile in `projects`. Die Sitzungen des Projekts
//!   bleiben und gelten danach als nicht zugeordnet.
//!
//! # Was nie dazugehört
//!
//! Der Projektordner (diese Crate kennt ihn nur als Text), die Audit-Kette
//! (`audit.jsonl`, `audit_anchors`; keine Anweisung hier nennt sie), Regeln
//! (`rules_snapshot`), die globale Konfiguration und das Agent-Depot.
//!
//! # In Gebrauch
//!
//! Die laufende Sitzung kommt in keinen Plan, auch nicht mit „alles". Ein
//! Projekt, dessen Sitzung läuft oder dessen Änderungen ohne ausdrückliche
//! Wahl warten, behält Projektwahl und Registereintrag; der Plan nennt das
//! in [`CleanupPlan::kept`].

use std::collections::{BTreeMap, BTreeSet};
use std::time::SystemTime;

use humanitl_core::{ProjectId, SessionId};
use rusqlite::Connection;
use sha2::{Digest as _, Sha256};

use crate::blob::{BlobStore, hex_encode};
use crate::error::{RecorderError, storage_failed};
use crate::projects::ProjectRow;
use crate::types::millis;

/// Kennzeichnet die Fassung des Digests; eine andere Rechnung bekommt eine
/// andere Zeile, damit ein alter Digest nie zufällig passt.
const DIGEST_TAG: &str = "humanitl cleanup plan v1";

/// Eine Art von Eintrag im Plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CleanupKind {
    /// Flüsse und Bodies der Sitzungen eines Projekts.
    RecordedFlows,
    /// Die wartende Schicht einer Wegwerf-Sandbox.
    WaitingChanges,
    /// Die gespeicherte Projektwahl.
    ProjectChoice,
    /// Der Eintrag im Projektregister.
    RegisterEntry,
}

impl CleanupKind {
    /// Jede Art, die dieser Daemon kennt, in der Reihenfolge der Anzeige.
    pub const ALL: [Self; 4] = [
        Self::RecordedFlows,
        Self::WaitingChanges,
        Self::ProjectChoice,
        Self::RegisterEntry,
    ];

    /// Der Name auf der Leitung.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RecordedFlows => "recorded_flows",
            Self::WaitingChanges => "waiting_changes",
            Self::ProjectChoice => "project_choice",
            Self::RegisterEntry => "register_entry",
        }
    }
}

/// Wessen Daten ein Plan betrifft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CleanupTarget {
    /// Diese Projekte, aktuell oder ersetzt.
    Projects(Vec<ProjectId>),
    /// Jedes Projekt des Registers; was in Gebrauch ist, bleibt.
    All,
    /// Die Sitzungen ohne Projekt („nicht zugeordnet"), und nur sie.
    Unassigned,
    /// Projektwahl und Registereintrag dieses einen Projekts.
    Forget(ProjectId),
}

/// Die Auswahl eines Plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupScope {
    /// Wessen Daten.
    pub target: CleanupTarget,
    /// Nur, was vor diesem Zeitpunkt liegt: Flüsse, die davor ankamen, und
    /// Wahl und Eintrag von Projekten, deren letzter Start davor liegt. Auf
    /// Millisekunden genau, wie die Datenbank rechnet.
    pub before: Option<SystemTime>,
    /// Die wartende Schicht einer Wegwerf-Sandbox gehört dazu; ohne diese
    /// ausdrückliche Wahl nie.
    pub include_waiting: bool,
}

/// Die wartende Schicht einer Wegwerf-Sandbox, wie der Daemon sie kennt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingLayer {
    /// Das Projekt des Laufs.
    pub project: ProjectId,
    /// Die Kennung der Schicht (die des Sandbox-Laufs).
    pub layer: String,
    /// Wie viele Änderungen warten.
    pub count: u64,
    /// Wie viele Bytes sie tragen.
    pub bytes: u64,
    /// Ob die Liste der Änderungen vollständig ist: nicht gekürzt und ohne
    /// Stelle, die sich nicht lesen ließ. Eine unvollständige Schicht darf
    /// kein Aufräumen verwerfen; der Daemon lehnt sie ab (`SANDBOX_054`).
    pub complete: bool,
}

/// Was gerade läuft oder wartet; das weiß nur der Daemon, nicht die
/// Datenbank.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LiveState {
    /// Die Sitzung, deren Sandbox läuft.
    pub running_session: Option<SessionId>,
    /// Das Projekt, in dem sie läuft.
    pub running_project: Option<ProjectId>,
    /// Die wartende Schicht, falls es eine gibt.
    pub waiting: Option<WaitingLayer>,
}

/// Ein Eintrag des Plans.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupItem {
    /// Die Art.
    pub kind: CleanupKind,
    /// Das Projekt; `None` für die nicht zugeordneten Sitzungen.
    pub project: Option<ProjectId>,
    /// Der Pfad des Projekts, ungesäubert; leer ohne Projekt.
    pub path: String,
    /// Der Ordnername des Projekts, ungesäubert.
    pub name: String,
    /// Wie viele Sitzungen der Eintrag berührt.
    pub sessions: u64,
    /// Wie viele Dinge fallen: Flüsse, Änderungen, eine Wahl, ein Eintrag.
    pub count: u64,
    /// Ungefähr so viele Bytes werden frei: Zeilen und Blobs.
    pub bytes: u64,
    /// Die Flüsse dieses Eintrags.
    pub(crate) flows: Vec<String>,
    /// Die Blobs, die mit diesem Eintrag fallen, mit ihrer Größe im Plan.
    pub(crate) blobs: Vec<([u8; 32], u64)>,
    /// Was der Digest von diesem Eintrag bindet.
    pub(crate) identity: Vec<String>,
}

impl CleanupItem {
    /// Die Blobs, die mit diesem Eintrag fallen, mit ihrer Größe im Plan.
    pub(crate) fn blobs(&self) -> &[([u8; 32], u64)] {
        &self.blobs
    }
}

/// Ein Teil der Auswahl, der bleibt, weil er in Gebrauch ist oder eine
/// ausdrückliche Wahl fehlt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kept {
    /// Was bleibt.
    pub kind: CleanupKind,
    /// Das Projekt; `None` für die nicht zugeordneten Sitzungen.
    pub project: Option<ProjectId>,
    /// Der Pfad, ungesäubert.
    pub path: String,
    /// Warum.
    pub reason: KeptReason,
}

/// Warum ein Teil der Auswahl bleibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeptReason {
    /// Die Sitzung läuft.
    Running,
    /// Änderungen warten, und die ausdrückliche Wahl fehlt.
    ChangesWaiting,
}

/// Ein Plan: die Einträge, was bleibt, und der Digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupPlan {
    /// Die Einträge, je Projekt in der Reihenfolge von [`CleanupKind::ALL`].
    pub items: Vec<CleanupItem>,
    /// Was die Auswahl träfe, aber bleibt.
    pub kept: Vec<Kept>,
    /// Der Digest über Auswahl und Einträge, hexadezimal.
    pub digest: String,
}

/// Was mit einem Eintrag geschah.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanupOutcome {
    /// Die Art.
    pub kind: CleanupKind,
    /// Das Projekt.
    pub project: Option<ProjectId>,
    /// Ob er gelöscht ist.
    pub deleted: bool,
    /// Wie viele Dinge fielen.
    pub count: u64,
    /// Wie viele Bytes frei wurden: Zeilen und entfernte Blobs.
    pub bytes: u64,
    /// Warum nicht oder nicht ganz; leer, wenn alles fiel.
    pub reason: Option<String>,
}

/// Was die Ausführung eines Plans ergab (`execute`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Executed {
    /// Der Plan ist nicht mehr derselbe; nichts ist gelöscht. Trägt den Plan,
    /// wie er jetzt aussieht.
    Stale(CleanupPlan),
    /// Gelöscht, wie geplant.
    Done {
        /// Der ausgeführte Plan.
        plan: CleanupPlan,
        /// Je Eintrag des Plans das Ergebnis, in derselben Reihenfolge.
        outcomes: Vec<CleanupOutcome>,
    },
}

/// Berechnet den Plan. Läuft im Schreib-Thread, damit er dieselben Zeilen
/// sieht wie die Ausführung danach.
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn plan(
    conn: &Connection,
    blobs: &BlobStore,
    scope: &CleanupScope,
    live: &LiveState,
) -> Result<CleanupPlan, RecorderError> {
    let before = scope.before.map(millis);
    let mut items = Vec::new();
    let mut kept = Vec::new();
    match &scope.target {
        CleanupTarget::Unassigned => {
            let sessions = select_sessions(conn, None, live, &mut kept)?;
            push_flows(conn, &mut items, None, &sessions, before)?;
        }
        target => {
            for row in projects_of(conn, target)? {
                plan_project(conn, scope, live, before, &row, &mut items, &mut kept)?;
            }
        }
    }
    attach_blobs(conn, blobs, &mut items)?;
    let digest = digest_of(scope, &items);
    Ok(CleanupPlan {
        items,
        kept,
        digest,
    })
}

/// Führt einen Plan aus, aber nur, wenn er noch derselbe ist: In einer
/// Transaktion wird er mit derselben Auswahl neu berechnet und sein Digest
/// mit `digest` verglichen; nur bei Gleichheit wird gelöscht, noch in
/// derselben Transaktion. Die Blob-Dateien entfernt der Aufrufer nach dem
/// Commit ([`Executed::Done`] nennt sie über die Einträge des Plans).
///
/// # Errors
///
/// [`RecorderError::Storage`], wenn eine Anweisung scheitert; dann ist
/// nichts gelöscht.
pub fn execute(
    conn: &Connection,
    blobs: &BlobStore,
    scope: &CleanupScope,
    live: &LiveState,
    digest: &str,
) -> Result<Executed, RecorderError> {
    conn.execute_batch("BEGIN IMMEDIATE;")
        .map_err(|err| storage_failed(format!("could not begin the cleanup ({err})")))?;
    let result = plan(conn, blobs, scope, live).and_then(|now| {
        if now.digest != digest.trim() {
            return Ok(Executed::Stale(now));
        }
        let outcomes = delete(conn, &now)?;
        Ok(Executed::Done {
            plan: now,
            outcomes,
        })
    });
    match result {
        Ok(Executed::Done { plan, outcomes }) => {
            conn.execute_batch("COMMIT;").map_err(|err| {
                let _ignored = conn.execute_batch("ROLLBACK;");
                storage_failed(format!("could not commit the cleanup ({err})"))
            })?;
            Ok(Executed::Done { plan, outcomes })
        }
        Ok(stale) => {
            let _ignored = conn.execute_batch("ROLLBACK;");
            Ok(stale)
        }
        Err(err) => {
            let _ignored = conn.execute_batch("ROLLBACK;");
            Err(err)
        }
    }
}

/// Die Zeilen der Auswahl, in fester Reihenfolge.
fn projects_of(
    conn: &Connection,
    target: &CleanupTarget,
) -> Result<Vec<ProjectRow>, RecorderError> {
    match target {
        CleanupTarget::Projects(ids) => {
            let mut seen = BTreeSet::new();
            let mut rows = Vec::new();
            for id in ids {
                if seen.insert(*id)
                    && let Some(row) = crate::projects::get(conn, *id)?
                {
                    rows.push(row);
                }
            }
            Ok(rows)
        }
        CleanupTarget::Forget(id) => Ok(crate::projects::get(conn, *id)?.into_iter().collect()),
        CleanupTarget::All => crate::projects::all_rows(conn),
        CleanupTarget::Unassigned => Ok(Vec::new()),
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_project(
    conn: &Connection,
    scope: &CleanupScope,
    live: &LiveState,
    before: Option<i64>,
    row: &ProjectRow,
    items: &mut Vec<CleanupItem>,
    kept: &mut Vec<Kept>,
) -> Result<(), RecorderError> {
    let forget = matches!(scope.target, CleanupTarget::Forget(_));
    let waiting = live
        .waiting
        .as_ref()
        .filter(|waiting| waiting.project == row.id);
    if !forget {
        let sessions = select_sessions(conn, Some(row), live, kept)?;
        push_flows(conn, items, Some(row), &sessions, before)?;
        if let Some(waiting) = waiting {
            if scope.include_waiting {
                items.push(CleanupItem {
                    kind: CleanupKind::WaitingChanges,
                    project: Some(row.id),
                    path: row.path.clone(),
                    name: row.display_name.clone(),
                    sessions: 0,
                    count: waiting.count,
                    bytes: waiting.bytes,
                    flows: Vec::new(),
                    blobs: Vec::new(),
                    identity: vec![
                        project_line(row),
                        format!(
                            "layer {} count {} bytes {} complete {}",
                            waiting.layer, waiting.count, waiting.bytes, waiting.complete
                        ),
                    ],
                });
            } else {
                kept.push(kept_of(
                    row,
                    CleanupKind::WaitingChanges,
                    KeptReason::ChangesWaiting,
                ));
            }
        }
    }
    let old_enough = forget
        || before.is_none_or(|before| millis(row.last_started.unwrap_or(row.first_seen)) < before);
    if !old_enough {
        return Ok(());
    }
    // Ein Projekt in Gebrauch behält Wahl und Eintrag: Sein Lauf schriebe
    // sonst in eine Zeile, die es nicht mehr gibt, und wartende Änderungen
    // ohne ausdrückliche Wahl gehörten weiter zu einem bekannten Projekt.
    let in_use = if live.running_project == Some(row.id) {
        Some(KeptReason::Running)
    } else if waiting.is_some() && (forget || !scope.include_waiting) {
        Some(KeptReason::ChangesWaiting)
    } else {
        None
    };
    if let Some(reason) = in_use {
        if row.choice.is_some() {
            kept.push(kept_of(row, CleanupKind::ProjectChoice, reason));
        }
        kept.push(kept_of(row, CleanupKind::RegisterEntry, reason));
        return Ok(());
    }
    if let Some(choice) = row.choice.as_deref() {
        items.push(CleanupItem {
            kind: CleanupKind::ProjectChoice,
            project: Some(row.id),
            path: row.path.clone(),
            name: row.display_name.clone(),
            sessions: 0,
            count: 1,
            bytes: u64::try_from(choice.len()).unwrap_or(u64::MAX),
            flows: Vec::new(),
            blobs: Vec::new(),
            identity: vec![
                project_line(row),
                format!("choice {}", hex_encode(&Sha256::digest(choice.as_bytes()))),
            ],
        });
    }
    let sessions = count_sessions(conn, row.id)?;
    items.push(CleanupItem {
        kind: CleanupKind::RegisterEntry,
        project: Some(row.id),
        path: row.path.clone(),
        name: row.display_name.clone(),
        sessions,
        count: 1,
        bytes: u64::try_from(row.path.len() + row.display_name.len()).unwrap_or(u64::MAX),
        flows: Vec::new(),
        blobs: Vec::new(),
        identity: vec![project_line(row), format!("sessions {sessions}")],
    });
    Ok(())
}

fn project_line(row: &ProjectRow) -> String {
    format!("project {} version {}", row.id, row.version)
}

fn kept_of(row: &ProjectRow, kind: CleanupKind, reason: KeptReason) -> Kept {
    Kept {
        kind,
        project: Some(row.id),
        path: row.path.clone(),
        reason,
    }
}

/// Die Sitzungen eines Projekts (oder ohne Projekt) in der Auswahl, ohne die
/// laufende; die landet in `kept`.
fn select_sessions(
    conn: &Connection,
    row: Option<&ProjectRow>,
    live: &LiveState,
    kept: &mut Vec<Kept>,
) -> Result<Vec<String>, RecorderError> {
    let sql = match row {
        Some(_) => "SELECT id FROM sessions WHERE project_id = ?1 ORDER BY id",
        None => "SELECT id FROM sessions WHERE project_id IS NULL AND ?1 IS NULL ORDER BY id",
    };
    let project = row.map(|row| row.id.to_string());
    let ids = strings(conn, sql, rusqlite::params![project])?;
    let running = live.running_session.map(|id| id.to_string());
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if running.as_deref() == Some(id.as_str()) {
            kept.push(Kept {
                kind: CleanupKind::RecordedFlows,
                project: row.map(|row| row.id),
                path: row.map(|row| row.path.clone()).unwrap_or_default(),
                reason: KeptReason::Running,
            });
            continue;
        }
        out.push(id);
    }
    Ok(out)
}

/// Ein Eintrag der Flüsse dieser Sitzungen, falls es welche gibt; mit einer
/// Grenze nur die Flüsse, die davor ankamen. Die Sitzungen stehen alle im
/// Digest, auch ohne Fluss: Eine neue Sitzung im Projekt ist ein anderer
/// Plan.
fn push_flows(
    conn: &Connection,
    items: &mut Vec<CleanupItem>,
    row: Option<&ProjectRow>,
    sessions: &[String],
    before: Option<i64>,
) -> Result<(), RecorderError> {
    let mut flows = Vec::new();
    for session in sessions {
        flows.extend(strings(
            conn,
            "SELECT id FROM flows WHERE session_id = ?1 AND (?2 IS NULL OR ts < ?2) ORDER BY id",
            rusqlite::params![session, before],
        )?);
    }
    if flows.is_empty() {
        return Ok(());
    }
    flows.sort();
    let mut identity: Vec<String> = row.map(project_line).into_iter().collect();
    identity.extend(sessions.iter().map(|id| format!("session {id}")));
    let mut bytes = 0_u64;
    for flow in &flows {
        identity.push(flow_line(conn, flow)?);
        bytes = bytes.saturating_add(row_bytes(conn, flow)?);
    }
    items.push(CleanupItem {
        kind: CleanupKind::RecordedFlows,
        project: row.map(|row| row.id),
        path: row.map(|row| row.path.clone()).unwrap_or_default(),
        name: row.map(|row| row.display_name.clone()).unwrap_or_default(),
        sessions: u64::try_from(sessions.len()).unwrap_or(u64::MAX),
        count: u64::try_from(flows.len()).unwrap_or(u64::MAX),
        bytes,
        flows,
        blobs: Vec::new(),
        identity,
    });
    Ok(())
}

/// Die Identität eines Flusses mit allem, was mit ihm fällt: Zustand und
/// Entscheidung, je Nachricht Richtung, Größe, Body (Blob-Hash oder SHA-256
/// des Bodys in der Datenbank), SHA-256 der Kopfzeilen und Kürzung, und je
/// Tabelle der Funde und Befunde Anzahl und höchster Index. Kommt nach dem
/// Plan eine Zeile dazu oder ändert sich eine, auch bei gleicher Länge (eine
/// späte Antwort, ein ersetzter Body, ein Befund, eine Entscheidung), ist es
/// ein anderer Plan.
fn flow_line(conn: &Connection, flow: &str) -> Result<String, RecorderError> {
    let failed =
        |err: rusqlite::Error| storage_failed(format!("could not read the flow {flow} ({err})"));
    let (state, decision, findings, diagnostics) = conn
        .query_row(
            "SELECT state, COALESCE(decision, '-'), \
             (SELECT COUNT(*) || '/' || COALESCE(MAX(idx), -1) FROM findings WHERE flow_id = ?1), \
             (SELECT COUNT(*) || '/' || COALESCE(MAX(idx), -1) FROM diagnostics \
              WHERE flow_id = ?1) \
             FROM flows WHERE id = ?1",
            [flow],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            },
        )
        .map_err(failed)?;
    let mut statement = conn
        .prepare(
            "SELECT dir, size, blob_sha256, body_inline, headers_json, truncated FROM messages \
             WHERE flow_id = ?1 ORDER BY dir",
        )
        .map_err(failed)?;
    let rows = statement
        .query_map([flow], |row| {
            let body = if let Some(blob) = row.get::<_, Option<Vec<u8>>>(2)? {
                format!("blob {}", hex_encode(&blob))
            } else {
                let inline = row.get::<_, Option<Vec<u8>>>(3)?.unwrap_or_default();
                format!("inline {}", hex_encode(&Sha256::digest(&inline)))
            };
            Ok(format!(
                "{} size {} {body} headers {} truncated {}",
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                hex_encode(&Sha256::digest(row.get::<_, String>(4)?.as_bytes())),
                row.get::<_, i64>(5)?,
            ))
        })
        .map_err(failed)?;
    let mut messages = Vec::new();
    for row in rows {
        messages.push(row.map_err(failed)?);
    }
    Ok(format!(
        "flow {flow} state {state} decision {decision} messages [{}] findings {findings} \
         diagnostics {diagnostics}",
        messages.join("; ")
    ))
}

/// Ungefähr die Bytes der Zeilen eines Flusses: Kopfzeilen und Bodies in der
/// Datenbank, Ziel und Pfad. Nicht die Dateigröße der Datenbank; `SQLite`
/// gibt den Platz erst mit `VACUUM` frei.
fn row_bytes(conn: &Connection, flow: &str) -> Result<u64, RecorderError> {
    let bytes: i64 = conn
        .query_row(
            "SELECT COALESCE((SELECT SUM(LENGTH(headers_json) + COALESCE(LENGTH(body_inline), 0)) \
             FROM messages WHERE flow_id = ?1), 0) \
             + COALESCE((SELECT LENGTH(host) + LENGTH(path) FROM flows WHERE id = ?1), 0)",
            [flow],
            |row| row.get(0),
        )
        .map_err(|err| storage_failed(format!("could not size the flow {flow} ({err})")))?;
    Ok(u64::try_from(bytes).unwrap_or(0))
}

/// Ordnet jedem Eintrag der Flüsse seine Blobs zu: Jeder Blob, auf den einer
/// seiner Flüsse zeigt, steht im Digest, und ob er fällt, auch. Er fällt nur,
/// wenn jede Zeile, die ihn nennt, zu einem Fluss des Plans gehört; seine
/// Bytes zählen beim ersten Eintrag, der ihn nennt.
fn attach_blobs(
    conn: &Connection,
    blobs: &BlobStore,
    items: &mut [CleanupItem],
) -> Result<(), RecorderError> {
    let planned: BTreeSet<String> = items
        .iter()
        .flat_map(|item| item.flows.iter().cloned())
        .collect();
    let mut decided: BTreeMap<[u8; 32], bool> = BTreeMap::new();
    let mut counted: BTreeSet<[u8; 32]> = BTreeSet::new();
    for item in items.iter_mut() {
        let mut referenced = BTreeSet::new();
        for flow in &item.flows {
            referenced.extend(blob_refs(conn, flow)?);
        }
        for sha in referenced {
            let drops = if let Some(known) = decided.get(&sha) {
                *known
            } else {
                let users = strings(
                    conn,
                    "SELECT flow_id FROM messages WHERE blob_sha256 = ?1",
                    [sha.as_slice()],
                )?;
                let all_planned = users.iter().all(|flow| planned.contains(flow));
                decided.insert(sha, all_planned);
                all_planned
            };
            item.identity.push(format!(
                "blob {} {}",
                hex_encode(&sha),
                if drops { "drop" } else { "keep" }
            ));
            if drops && counted.insert(sha) {
                let len = std::fs::metadata(blobs.path(&sha)).map_or(0, |meta| meta.len());
                item.blobs.push((sha, len));
                item.bytes = item.bytes.saturating_add(len);
            }
        }
    }
    Ok(())
}

fn blob_refs(conn: &Connection, flow: &str) -> Result<Vec<[u8; 32]>, RecorderError> {
    let mut statement = conn
        .prepare("SELECT blob_sha256 FROM messages WHERE flow_id = ?1 AND blob_sha256 IS NOT NULL")
        .map_err(|err| storage_failed(format!("could not list the blobs of {flow} ({err})")))?;
    let rows = statement
        .query_map([flow], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|err| storage_failed(format!("could not read the blobs of {flow} ({err})")))?;
    let mut out = Vec::new();
    for row in rows {
        let bytes =
            row.map_err(|err| storage_failed(format!("could not read a blob reference ({err})")))?;
        if let Ok(sha) = <[u8; 32]>::try_from(bytes.as_slice()) {
            out.push(sha);
        }
    }
    Ok(out)
}

fn count_sessions(conn: &Connection, id: ProjectId) -> Result<u64, RecorderError> {
    let count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE project_id = ?1",
            [id.to_string()],
            |row| row.get(0),
        )
        .map_err(|err| storage_failed(format!("could not count the sessions of {id} ({err})")))?;
    Ok(u64::try_from(count).unwrap_or(0))
}

/// Der Digest: die Fassung, die Auswahl, und je Eintrag Art, Projekt und
/// alles, was seine Identität ausmacht. Größen und Zahlen stehen nicht darin;
/// sie folgen aus den Zeilen, die darin stehen.
fn digest_of(scope: &CleanupScope, items: &[CleanupItem]) -> String {
    let mut hasher = Sha256::new();
    let mut line = |text: &str| {
        hasher.update(text.as_bytes());
        hasher.update(b"\n");
    };
    line(DIGEST_TAG);
    line(&scope_line(scope));
    for item in items {
        line(&format!(
            "item {} {}",
            item.kind.as_str(),
            item.project
                .map_or_else(|| "-".to_owned(), |id| id.to_string())
        ));
        for part in &item.identity {
            line(part);
        }
    }
    hex_encode(&hasher.finalize())
}

/// Die Auswahl als eine Zeile, in fester Form.
fn scope_line(scope: &CleanupScope) -> String {
    let target = match &scope.target {
        CleanupTarget::Projects(ids) => {
            let ids: BTreeSet<String> = ids.iter().map(ToString::to_string).collect();
            format!("projects {}", ids.into_iter().collect::<Vec<_>>().join(","))
        }
        CleanupTarget::All => "all".to_owned(),
        CleanupTarget::Unassigned => "unassigned".to_owned(),
        CleanupTarget::Forget(id) => format!("forget {id}"),
    };
    format!(
        "scope {target} before {} waiting {}",
        scope
            .before
            .map_or_else(|| "-".to_owned(), |at| millis(at).to_string()),
        scope.include_waiting
    )
}

/// Löscht, was der Plan nennt, in der offenen Transaktion. Die wartende
/// Schicht liegt nicht hier; ihr Ergebnis trägt der Aufrufer nach.
fn delete(conn: &Connection, plan: &CleanupPlan) -> Result<Vec<CleanupOutcome>, RecorderError> {
    let mut outcomes = Vec::with_capacity(plan.items.len());
    for item in &plan.items {
        let deleted = match item.kind {
            CleanupKind::RecordedFlows => {
                for flow in &item.flows {
                    for sql in [
                        "DELETE FROM findings WHERE flow_id = ?1",
                        "DELETE FROM diagnostics WHERE flow_id = ?1",
                        "DELETE FROM messages WHERE flow_id = ?1",
                        "DELETE FROM flows WHERE id = ?1",
                    ] {
                        run(conn, sql, [flow.as_str()])?;
                    }
                }
                true
            }
            CleanupKind::WaitingChanges => false,
            CleanupKind::ProjectChoice => {
                let id = project_text(item)?;
                run(
                    conn,
                    "UPDATE projects SET choice = NULL, choice_saved_at = NULL, \
                     version = version + 1 WHERE id = ?1",
                    [id.as_str()],
                )?;
                true
            }
            CleanupKind::RegisterEntry => {
                let id = project_text(item)?;
                // Die Sitzungen bleiben; ohne ihr Projekt gelten sie als
                // nicht zugeordnet (HUM-532 Punkt 1a).
                run(
                    conn,
                    "UPDATE sessions SET project_id = NULL WHERE project_id = ?1",
                    [id.as_str()],
                )?;
                run(conn, "DELETE FROM projects WHERE id = ?1", [id.as_str()])?;
                true
            }
        };
        outcomes.push(CleanupOutcome {
            kind: item.kind,
            project: item.project,
            deleted,
            count: if deleted { item.count } else { 0 },
            // Die Blobs zählt der Aufrufer, wenn ihre Dateien weg sind.
            bytes: if deleted {
                item.bytes.saturating_sub(blob_share(item))
            } else {
                0
            },
            reason: None,
        });
    }
    Ok(outcomes)
}

/// Der Anteil der Blob-Dateien an [`CleanupItem::bytes`]; ihn rechnet erst
/// das Entfernen der Dateien gut.
fn blob_share(item: &CleanupItem) -> u64 {
    item.blobs
        .iter()
        .map(|(_sha, len)| *len)
        .fold(0, u64::saturating_add)
        .min(item.bytes)
}

fn project_text(item: &CleanupItem) -> Result<String, RecorderError> {
    item.project
        .map(|id| id.to_string())
        .ok_or_else(|| storage_failed("a cleanup item of a project names no project"))
}

fn run<P: rusqlite::Params>(conn: &Connection, sql: &str, params: P) -> Result<(), RecorderError> {
    conn.execute(sql, params)
        .map(|_rows| ())
        .map_err(|err| storage_failed(format!("could not clean up ({err}): {sql}")))
}

fn strings<P: rusqlite::Params>(
    conn: &Connection,
    sql: &str,
    params: P,
) -> Result<Vec<String>, RecorderError> {
    let mut statement = conn
        .prepare(sql)
        .map_err(|err| storage_failed(format!("could not prepare the cleanup ({err})")))?;
    let rows = statement
        .query_map(params, |row| row.get::<_, String>(0))
        .map_err(|err| storage_failed(format!("could not read for the cleanup ({err})")))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| storage_failed(format!("could not read a row ({err})")))?);
    }
    Ok(out)
}

impl crate::Recorder {
    /// Berechnet einen Aufräumplan im Schreib-Thread (`plan`).
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank oder wenn der
    /// Schreib-Thread nicht mehr da ist.
    pub async fn cleanup_plan(
        &self,
        scope: CleanupScope,
        live: LiveState,
    ) -> Result<CleanupPlan, RecorderError> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.send(crate::writer::WriterCmd::CleanupPlan {
            scope: Box::new(scope),
            live: Box::new(live),
            reply,
        });
        answer.await.map_err(|_closed| {
            storage_failed("the recorder thread ended before it could plan the cleanup")
        })?
    }

    /// Führt einen Aufräumplan aus, wenn sein Digest noch gilt (`execute`).
    ///
    /// Blockiert, bis der Schreib-Thread fertig ist; gedacht für einen
    /// blockierenden Aufrufer, der dabei eine Sperre hält (die wartende
    /// Schicht einer Wegwerf-Sandbox). Nie aus einem asynchronen Kontext.
    ///
    /// # Errors
    ///
    /// [`RecorderError::Storage`] bei einem Fehler der Datenbank oder wenn der
    /// Schreib-Thread nicht mehr da ist; dann ist nichts gelöscht.
    pub fn cleanup_blocking(
        &self,
        scope: CleanupScope,
        live: LiveState,
        digest: String,
    ) -> Result<Executed, RecorderError> {
        let (reply, answer) = tokio::sync::oneshot::channel();
        self.send(crate::writer::WriterCmd::Cleanup {
            scope: Box::new(scope),
            live: Box::new(live),
            digest,
            reply,
        });
        answer.blocking_recv().map_err(|_closed| {
            storage_failed("the recorder thread ended before it could run the cleanup")
        })?
    }
}
