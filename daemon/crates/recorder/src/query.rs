//! Die Lese-Seite: `ListFlows`, `GetFlow` und die Verbindungen dahinter.
//!
//! Gelesen wird nie über die Schreibverbindung. `rusqlite::Connection` ist
//! `!Sync`, und der Schreib-Thread soll nicht warten, während die Oberfläche
//! blättert. Stattdessen hält [`ReadPool`] eine Handvoll Nur-Lese-Verbindungen;
//! im WAL-Modus sieht jede von ihnen einen konsistenten Stand, ohne den
//! Schreiber zu blockieren (`https://www.sqlite.org/wal.html`).
//!
//! # Blättern ohne Offset
//!
//! Eine Seite endet bei der letzten gelieferten Zeile, und die nächste beginnt
//! genau dahinter: `WHERE (ts, id) < (?, ?)` bei absteigender Sortierung. Der
//! Vergleich benutzt dieselbe Spaltenfolge wie der Index `flows_ts`, deshalb
//! kostet die zehnte Seite so viel wie die erste. `OFFSET` würde jedes Mal
//! alles davor lesen und beim Einfügen neuer Zeilen Duplikate liefern.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};

use bytes::Bytes;
use humanitl_core::{BodyRef, FlowId, SessionId};
use rusqlite::types::Type;
use rusqlite::{Connection, Row};

use crate::error::{RecorderError, cursor_mismatch, storage_failed};
use crate::filter::{self, Param};
use crate::types::{
    COUNT_CEILING, Cursor, CursorKey, Dir, FindingRecord, FlowDetail, FlowPage, FlowQuery,
    FlowSummary, MessageRecord, SortKey,
};

/// So viele Nur-Lese-Verbindungen bleiben zwischen zwei Abfragen offen.
const POOL_SIZE: usize = 8;

/// Die Spalten von `flows`, in der Reihenfolge von [`row_to_summary`].
const FLOW_COLUMNS: &str = "id, session_id, seq, ts, method, scheme, host, host_display, port, \
     path, upgrade, state, decision, block_reason, rule_id, passthrough, status, duration_ms, \
     held_ms, edited, findings_count, request_size, response_size, apex, catalog_id, error";

/// Ein kleiner Vorrat an Nur-Lese-Verbindungen.
///
/// Kein Wartezimmer: wer keine freie Verbindung findet, öffnet eine eigene.
/// `SQLite` verträgt beliebig viele Leser, und eine Abfrage soll nie darauf
/// warten, dass eine andere fertig wird.
#[derive(Debug)]
pub struct ReadPool {
    path: PathBuf,
    idle: Mutex<Vec<Connection>>,
}

impl ReadPool {
    /// Ein Vorrat für diese Datenbank.
    #[must_use]
    pub fn new(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            idle: Mutex::new(Vec::new()),
        }
    }

    /// Führt eine Abfrage auf einer Nur-Lese-Verbindung aus.
    ///
    /// # Errors
    ///
    /// Was der Aufrufer meldet, oder [`RecorderError::Open`], wenn sich keine
    /// Verbindung öffnen lässt.
    pub fn with<T, F>(&self, work: F) -> Result<T, RecorderError>
    where
        F: FnOnce(&Connection) -> Result<T, RecorderError>,
    {
        let pooled = self.take();
        let conn = match pooled {
            Some(conn) => conn,
            None => crate::schema::open_read(&self.path)?,
        };
        let result = work(&conn);
        self.give_back(conn);
        result
    }

    /// Nimmt eine freie Verbindung, falls es eine gibt.
    fn take(&self) -> Option<Connection> {
        let mut idle = self.idle.lock().unwrap_or_else(PoisonError::into_inner);
        idle.pop()
    }

    /// Legt eine Verbindung zurück, solange der Vorrat nicht voll ist.
    fn give_back(&self, conn: Connection) {
        let mut idle = self.idle.lock().unwrap_or_else(PoisonError::into_inner);
        if idle.len() < POOL_SIZE {
            idle.push(conn);
        }
    }
}

/// Beantwortet eine Anfrage an die Flow-Liste.
///
/// # Errors
///
/// [`RecorderError::Filter`] bei einem unlesbaren Filter,
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn list_flows(
    pool: &ReadPool,
    query: &FlowQuery,
    now_ms: i64,
) -> Result<FlowPage, RecorderError> {
    let filter = filter::parse(&query.filter, now_ms)?;
    let limit = query.effective_limit();
    check_cursor(query)?;

    pool.with(|conn| {
        let (total_estimate, capped) = count(conn, &filter)?;

        let mut where_sql = filter.sql.clone();
        let mut params = filter.params.clone();
        if let Some(cursor) = &query.cursor {
            let (fragment, values) = keyset(query.sort, query.desc, cursor);
            where_sql.push_str(" AND ");
            where_sql.push_str(&fragment);
            params.extend(values);
        }
        params.push(Param::Int(i64::from(limit)));

        let sql = format!(
            "SELECT {FLOW_COLUMNS} FROM flows WHERE {where_sql} ORDER BY {} LIMIT ?",
            order_by(query.sort, query.desc)
        );

        let mut statement = conn
            .prepare(&sql)
            .map_err(|err| storage_failed(format!("could not prepare the flow list ({err})")))?;
        let rows = statement
            .query_map(rusqlite::params_from_iter(params.iter()), row_to_summary)
            .map_err(|err| storage_failed(format!("could not run the flow list ({err})")))?;

        let mut out = Vec::with_capacity(limit as usize);
        for row in rows {
            out.push(
                row.map_err(|err| storage_failed(format!("could not read a flow row ({err})")))?,
            );
        }

        let next = if out.len() == limit as usize {
            out.last().map(|last| cursor_for(query.sort, last))
        } else {
            None
        };

        Ok(FlowPage {
            rows: out,
            next,
            total_estimate,
            capped,
        })
    })
}

/// Liefert einen Flow mit Nachrichten und Funden.
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn get_flow(pool: &ReadPool, id: FlowId) -> Result<Option<FlowDetail>, RecorderError> {
    let key = id.to_string();
    pool.with(|conn| {
        let sql = format!("SELECT {FLOW_COLUMNS} FROM flows WHERE id = ?");
        let summary = conn
            .query_row(&sql, [&key], row_to_summary)
            .map(Some)
            .or_else(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(storage_failed(format!(
                    "could not read the flow {key} ({other})"
                ))),
            })?;
        let Some(summary) = summary else {
            return Ok(None);
        };

        let messages = messages_of(conn, &key)?;
        let findings = findings_of(conn, &key)?;
        Ok(Some(FlowDetail {
            summary,
            messages,
            findings,
        }))
    })
}

/// Alle `blob_sha256`, auf die eine Zeile zeigt.
///
/// Grundlage für das Aufräumen verwaister Dateien beim Öffnen.
///
/// # Errors
///
/// [`RecorderError::Storage`] bei einem Fehler der Datenbank.
pub fn referenced_blobs(conn: &Connection) -> Result<HashSet<[u8; 32]>, RecorderError> {
    let mut statement = conn
        .prepare("SELECT blob_sha256 FROM messages WHERE blob_sha256 IS NOT NULL")
        .map_err(|err| storage_failed(format!("could not prepare the blob census ({err})")))?;
    let rows = statement
        .query_map([], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|err| storage_failed(format!("could not run the blob census ({err})")))?;
    let mut out = HashSet::new();
    for row in rows {
        let bytes =
            row.map_err(|err| storage_failed(format!("could not read a blob reference ({err})")))?;
        if let Ok(sha256) = <[u8; 32]>::try_from(bytes.as_slice()) {
            out.insert(sha256);
        }
    }
    Ok(out)
}

/// Die Nachrichten eines Flows.
fn messages_of(conn: &Connection, flow: &str) -> Result<Vec<MessageRecord>, RecorderError> {
    let mut statement = conn
        .prepare(
            "SELECT dir, headers_json, content_type, content_encoding, body_inline, blob_sha256, \
             size, truncated FROM messages WHERE flow_id = ? ORDER BY dir",
        )
        .map_err(|err| storage_failed(format!("could not prepare the messages ({err})")))?;
    let rows = statement
        .query_map([flow], row_to_message)
        .map_err(|err| storage_failed(format!("could not run the messages ({err})")))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| {
            storage_failed(format!(
                "could not read a message of the flow {flow} ({err})"
            ))
        })?);
    }
    Ok(out)
}

/// Die Funde eines Flows.
fn findings_of(conn: &Connection, flow: &str) -> Result<Vec<FindingRecord>, RecorderError> {
    let mut statement = conn
        .prepare(
            "SELECT idx, kind, location, span_start, span_end, tier, value_hash, display_prefix, \
             resolved FROM findings WHERE flow_id = ? ORDER BY idx",
        )
        .map_err(|err| storage_failed(format!("could not prepare the findings ({err})")))?;
    let rows = statement
        .query_map([flow], row_to_finding)
        .map_err(|err| storage_failed(format!("could not run the findings ({err})")))?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row.map_err(|err| {
            storage_failed(format!(
                "could not read a finding of the flow {flow} ({err})"
            ))
        })?);
    }
    Ok(out)
}

/// Zählt die Treffer, aber höchstens bis [`COUNT_CEILING`].
///
/// Gezählt wird eine Zeile mehr als die Obergrenze; kommt sie zurück, gibt es
/// mehr Treffer als gezählt, und der zweite Rückgabewert sagt das. Die Zahl
/// selbst wird auf die Obergrenze gestutzt, damit niemand `10001` für eine
/// exakte Zahl hält (`backlog/CONVENTIONS.md` 4.13).
fn count(conn: &Connection, filter: &filter::Filter) -> Result<(u64, bool), RecorderError> {
    let sql = format!(
        "SELECT COUNT(*) FROM (SELECT 1 FROM flows WHERE {} LIMIT {})",
        filter.sql,
        COUNT_CEILING + 1
    );
    let total: i64 = conn
        .query_row(
            &sql,
            rusqlite::params_from_iter(filter.params.iter()),
            |row| row.get(0),
        )
        .map_err(|err| storage_failed(format!("could not count the flows ({err})")))?;
    let total = u64::try_from(total).unwrap_or(0);
    if total > COUNT_CEILING {
        Ok((COUNT_CEILING, true))
    } else {
        Ok((total, false))
    }
}

/// Die `ORDER BY`-Klausel zur Sortierung.
fn order_by(sort: SortKey, desc: bool) -> String {
    let dir = if desc { "DESC" } else { "ASC" };
    match sort.expr() {
        Some(expr) => format!("{expr} {dir}, ts {dir}, id {dir}"),
        None => format!("ts {dir}, id {dir}"),
    }
}

/// Lehnt einen Cursor ab, der nicht zur Sortierung passt.
///
/// Ein Cursor ohne Sortierwert kann nur nach `ts` weiterblättern. Käme er mit
/// `sort: Host`, filterte [`keyset`] nach `(ts, id)`, während [`order_by`] nach
/// `host` sortiert: die Seiten hätten Lücken und Wiederholungen. Das ist ein
/// Fehler des Aufrufers und wird als solcher gemeldet, statt still falsche
/// Seiten zu liefern.
///
/// # Errors
///
/// [`RecorderError::Filter`] mit `RECORDER_002`.
fn check_cursor(query: &FlowQuery) -> Result<(), RecorderError> {
    let Some(cursor) = &query.cursor else {
        return Ok(());
    };
    if query.sort != SortKey::Ts && cursor.sort.is_none() {
        return Err(cursor_mismatch(format!(
            "the cursor for the flow list carries no value for the sort key {}; pass the cursor \
             that came back from the previous page unchanged, or sort by ts",
            query.sort.as_str()
        )));
    }
    if query.sort == SortKey::Ts && cursor.sort.is_some() {
        return Err(cursor_mismatch(
            "the cursor for the flow list carries a value for a sort key, but the list is sorted \
             by ts; pass the cursor that came back from the previous page unchanged",
        ));
    }
    Ok(())
}

/// Die Keyset-Bedingung, die genau hinter dem Cursor weitermacht.
///
/// [`check_cursor`] hat vorher sichergestellt, dass Cursor und Sortierung
/// zusammenpassen; der letzte Zweig ist deshalb nur noch der Fall
/// [`SortKey::Ts`].
fn keyset(sort: SortKey, desc: bool, cursor: &Cursor) -> (String, Vec<Param>) {
    let cmp = if desc { "<" } else { ">" };
    match (sort.expr(), &cursor.sort) {
        (Some(expr), Some(key)) => {
            let value = match key {
                CursorKey::Int(number) => Param::Int(*number),
                CursorKey::Text(text) => Param::Text(text.clone()),
            };
            (
                format!("({expr}, ts, id) {cmp} (?, ?, ?)"),
                vec![value, Param::Int(cursor.ts), Param::Text(cursor.id.clone())],
            )
        }
        _ => (
            format!("(ts, id) {cmp} (?, ?)"),
            vec![Param::Int(cursor.ts), Param::Text(cursor.id.clone())],
        ),
    }
}

/// Der Cursor, der hinter dieser Zeile weitermacht.
fn cursor_for(sort: SortKey, row: &FlowSummary) -> Cursor {
    let key = match sort {
        SortKey::Ts => None,
        SortKey::Host => Some(CursorKey::Text(row.host.clone())),
        SortKey::Duration => Some(CursorKey::Int(row.duration_ms.unwrap_or(-1))),
        SortKey::Size => Some(CursorKey::Int(
            i64::try_from(row.request_size)
                .unwrap_or(i64::MAX)
                .saturating_add(i64::try_from(row.response_size.unwrap_or(0)).unwrap_or(i64::MAX)),
        )),
    };
    Cursor {
        ts: row.ts,
        id: row.id.to_string(),
        sort: key,
    }
}

/// Eine Zeile aus `flows`.
fn row_to_summary(row: &Row<'_>) -> rusqlite::Result<FlowSummary> {
    let id: String = row.get(0)?;
    let session: String = row.get(1)?;
    Ok(FlowSummary {
        id: FlowId::parse(&id).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(0, Type::Text, Box::new(err))
        })?,
        session: SessionId::parse(&session).map_err(|err| {
            rusqlite::Error::FromSqlConversionFailure(1, Type::Text, Box::new(err))
        })?,
        seq: row.get(2)?,
        ts: row.get(3)?,
        method: row.get(4)?,
        scheme: row.get(5)?,
        host: row.get(6)?,
        host_display: row.get(7)?,
        port: u16::try_from(row.get::<_, i64>(8)?).unwrap_or(0),
        path: row.get(9)?,
        upgrade: row.get(10)?,
        state: row.get(11)?,
        decision: row.get(12)?,
        block_reason: row.get(13)?,
        rule_id: row.get(14)?,
        passthrough: row.get::<_, i64>(15)? != 0,
        status: row
            .get::<_, Option<i64>>(16)?
            .and_then(|value| u16::try_from(value).ok()),
        duration_ms: row.get(17)?,
        held_ms: row.get(18)?,
        edited: row.get::<_, i64>(19)? != 0,
        findings_count: u32::try_from(row.get::<_, i64>(20)?).unwrap_or(u32::MAX),
        request_size: u64::try_from(row.get::<_, i64>(21)?).unwrap_or(0),
        response_size: row
            .get::<_, Option<i64>>(22)?
            .map(|value| u64::try_from(value).unwrap_or(0)),
        apex: row.get(23)?,
        catalog_id: row.get(24)?,
        error: row.get(25)?,
    })
}

/// Eine Zeile aus `messages`.
fn row_to_message(row: &Row<'_>) -> rusqlite::Result<MessageRecord> {
    let dir: String = row.get(0)?;
    let headers_json: String = row.get(1)?;
    let content_type: Option<String> = row.get(2)?;
    let content_encoding: Option<String> = row.get(3)?;
    let inline: Option<Vec<u8>> = row.get(4)?;
    let blob: Option<Vec<u8>> = row.get(5)?;
    let size = u64::try_from(row.get::<_, i64>(6)?).unwrap_or(0);
    let truncated = row.get::<_, i64>(7)? != 0;

    let inline = inline.map(Bytes::from);
    let sha256 = blob
        .as_deref()
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
        .or_else(|| {
            inline
                .as_ref()
                .map(|bytes| humanitl_core::http::sha256(bytes))
        })
        .unwrap_or_default();

    let body = BodyRef {
        sha256,
        size,
        inline,
        content_type: content_type.clone(),
        truncated,
    };

    Ok(MessageRecord {
        dir: Dir::parse(&dir).unwrap_or(Dir::Request),
        headers: decode_headers(&headers_json),
        content_type,
        content_encoding,
        body,
    })
}

/// Eine Zeile aus `findings`.
fn row_to_finding(row: &Row<'_>) -> rusqlite::Result<FindingRecord> {
    let hash: Vec<u8> = row.get(6)?;
    Ok(FindingRecord {
        idx: u32::try_from(row.get::<_, i64>(0)?).unwrap_or(0),
        kind: row.get(1)?,
        location: row.get(2)?,
        span_start: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
        span_end: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
        tier: row.get(5)?,
        value_hash: <[u8; 32]>::try_from(hash.as_slice()).unwrap_or_default(),
        display_prefix: row.get(7)?,
        resolved: row.get(8)?,
    })
}

/// Liest `[["name","value"],…]` zurück.
///
/// Eine unlesbare Zeile liefert eine leere Liste statt eines Fehlers: der Flow
/// selbst ist die wichtigere Information, und die Kopfzeilen sind hier nur
/// Anzeige.
fn decode_headers(json: &str) -> Vec<(String, String)> {
    let Ok(value) = serde_json::from_str::<Vec<Vec<String>>>(json) else {
        return Vec::new();
    };
    value
        .into_iter()
        .filter_map(|pair| {
            let mut iter = pair.into_iter();
            let name = iter.next()?;
            let value = iter.next().unwrap_or_default();
            Some((name, value))
        })
        .collect()
}
