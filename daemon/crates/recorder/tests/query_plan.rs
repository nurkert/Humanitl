//! Was `SQLite` mit den Abfragen der Flow-Liste vorhat.
//!
//! Ein Index nützt nur, wenn der Planer ihn auch benutzt. Diese Tests lesen
//! deshalb `EXPLAIN QUERY PLAN` und halten fest, was gelten muss:
//!
//! - Die Zählung hinter dem Filter `host:` ist ein `SEARCH` über
//!   `flows_host_rev`, kein `SCAN`. Vor `V3__host_suffix.sql` war sie
//!   `SCAN flows USING COVERING INDEX flows_host`, also ein Lauf über die ganze
//!   History, gleich wie wenige Zeilen am Ende übrig blieben.
//! - Die Liste ohne Filter und die Seite hinter einem Cursor brauchen keinen
//!   temporären B-Baum mehr. Vorher sortierte `flows_ts` die Spalte `id`
//!   aufsteigend, die Abfrage absteigend, und `SQLite` meldete
//!   `USE TEMP B-TREE FOR LAST TERM OF ORDER BY`.
//!
//! Für die *Seite* eines `host:`-Filters wird bewusst kein bestimmter Weg
//! verlangt: über `flows_host_rev`, wenn der Host selten ist, über `flows_ts`
//! mit frühem Abbruch am `LIMIT`, wenn er häufig ist. Welcher der bessere ist,
//! weiß erst `ANALYZE` (`Recorder::analyze`), und beide sind richtig. Verboten
//! ist nur der Lauf über die Tabelle selbst.
//!
//! Die Zeichenketten des Planers sind nicht Teil einer stabilen Schnittstelle
//! von `SQLite`. Geprüft werden deshalb nur die Wörter, die die Aussage tragen:
//! `SEARCH`, der Indexname, und die Abwesenheit von `TEMP B-TREE`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::SystemTime;

use humanitl_core::SessionId;
use humanitl_recorder::{Recorder, RecorderSettings, SessionMeta, suffix_range};

/// Die Spalten, die `list_flows` liest.
const FLOW_COLUMNS: &str = "id, session_id, seq, ts, method, scheme, host, host_display, port, \
     path, upgrade, state, decision, block_reason, rule_id, passthrough, status, duration_ms, \
     held_ms, edited, findings_count, request_size, response_size, apex, catalog_id";

/// Eine migrierte, leere Datenbank samt Verbindung darauf.
fn database() -> (tempfile::TempDir, rusqlite::Connection) {
    let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
    let db = dir.path().join("humanitl.db");
    let recorder = Recorder::open(&db, &dir.path().join("blobs"), RecorderSettings::default())
        .unwrap_or_else(|err| panic!("{err}"));
    recorder.start_session(&SessionMeta {
        id: SessionId::new(),
        started_at: SystemTime::now(),
        sandbox_profile: "default".to_owned(),
        llm_endpoint: None,
        work_dir: "/tmp".to_owned(),
        agent: "opencode".to_owned(),
    });
    drop(recorder);
    let conn = rusqlite::Connection::open(&db).unwrap_or_else(|err| panic!("{err}"));
    (dir, conn)
}

/// Der Plan zu einer Abfrage, Zeile für Zeile.
fn plan(conn: &rusqlite::Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Vec<String> {
    let mut statement = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap_or_else(|err| panic!("{err}"));
    let rows = statement
        .query_map(params, |row| row.get::<_, String>(3))
        .unwrap_or_else(|err| panic!("{err}"));
    rows.map(Result::unwrap_or_default).collect()
}

#[test]
fn the_host_filter_seeks_into_the_index_instead_of_scanning_it() {
    let (_dir, conn) = database();
    let (low, high) = suffix_range("github.com");

    let count = plan(
        &conn,
        "SELECT COUNT(*) FROM (SELECT 1 FROM flows WHERE (host_rev >= ? AND host_rev < ?) \
         LIMIT 10001)",
        rusqlite::params![low, high],
    );
    let joined = count.join(" | ");
    assert!(
        joined.contains("SEARCH") && joined.contains("flows_host_rev"),
        "the count over a host filter must seek, the plan was: {joined}"
    );
    assert!(
        !joined.contains("SCAN flows"),
        "the count must not walk the whole table: {joined}"
    );

    // Die Seite darf beide Wege gehen: über `flows_host_rev`, wenn der Host
    // selten ist, über `flows_ts` mit frühem Abbruch am `LIMIT`, wenn er häufig
    // ist. Welcher der bessere ist, weiß erst `ANALYZE` (siehe
    // `Recorder::analyze`); verboten ist nur der Lauf über die Tabelle selbst.
    let page = plan(
        &conn,
        &format!(
            "SELECT {FLOW_COLUMNS} FROM flows WHERE (host_rev >= ? AND host_rev < ?) \
             ORDER BY ts DESC, id DESC LIMIT ?"
        ),
        rusqlite::params![low, high, 200_i64],
    );
    let joined = page.join(" | ");
    assert!(
        joined.contains("flows_host_rev") || joined.contains("flows_ts"),
        "the page of a host filter must read an index, the plan was: {joined}"
    );
    assert!(
        !page.iter().any(|line| line.trim() == "SCAN flows"),
        "the page must never walk the table itself: {joined}"
    );
}

#[test]
fn the_unfiltered_list_needs_no_temporary_sort() {
    let (_dir, conn) = database();
    let page = plan(
        &conn,
        &format!("SELECT {FLOW_COLUMNS} FROM flows WHERE 1 ORDER BY ts DESC, id DESC LIMIT ?"),
        rusqlite::params![200_i64],
    );
    let joined = page.join(" | ");
    assert!(
        joined.contains("flows_ts"),
        "the list must read the index flows_ts: {joined}"
    );
    assert!(
        !joined.contains("TEMP B-TREE"),
        "flows_ts(ts DESC, id DESC) must satisfy the order without a sorter: {joined}"
    );
}

#[test]
fn paging_by_timestamp_seeks_with_the_cursor() {
    let (_dir, conn) = database();
    let page = plan(
        &conn,
        &format!(
            "SELECT {FLOW_COLUMNS} FROM flows WHERE 1 AND (ts, id) < (?, ?) \
             ORDER BY ts DESC, id DESC LIMIT ?"
        ),
        rusqlite::params![1_i64, "zzz", 200_i64],
    );
    let joined = page.join(" | ");
    assert!(
        joined.contains("SEARCH") && joined.contains("flows_ts"),
        "the keyset cursor must seek into flows_ts: {joined}"
    );
    assert!(
        !joined.contains("TEMP B-TREE"),
        "the cursor page must not sort: {joined}"
    );
}

#[test]
fn the_index_of_the_migration_is_really_there() {
    let (_dir, conn) = database();
    let sql: String = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'index' AND name = 'flows_ts'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|err| panic!("{err}"));
    assert!(
        sql.contains("ts DESC") && sql.contains("id DESC"),
        "flows_ts must sort both columns descending: {sql}"
    );

    let columns: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM pragma_table_info('flows') WHERE name = 'host_rev'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);
    assert_eq!(columns, 1, "flows.host_rev is missing");
}
