//! Die Flow-Liste bei 100 000 Zeilen.
//!
//! Akzeptanzkriterium aus `backlog/sprint-2.md`, HUM-026: `list_flows` mit einem
//! `host:`-Filter antwortet unter 50 ms. Gemessen wird nur die Abfrage, nicht
//! das Befüllen; das Befüllen läuft deshalb über eine einzige Transaktion
//! direkt auf der Datenbank, nicht über den Schreib-Thread.
//!
//! Der Test ist absichtlich eigenständig: er baut sich die Zeilen mit demselben
//! Schema, das die Migration anlegt, und liest sie über die öffentliche
//! Schnittstelle zurück.
//!
//! # Warum `#[ignore]`
//!
//! Das Anlegen von 100 000 Zeilen mit fünf Indizes dauert im Debug-Profil rund
//! neun Sekunden, im Release-Profil rund vier. Zusammen mit `tests/recorder.rs`
//! risse das die Vorgabe „`cargo test -p humanitl-recorder` unter 20 s". Der
//! Test läuft deshalb auf Zuruf:
//!
//! ```sh
//! cargo test --release -p humanitl-recorder --test list_flows_scale -- --ignored --nocapture
//! ```
//!
//! Gemessen am 2026-09-03 (`rusqlite` 0.32, gebündeltes `SQLite` mit
//! `SQLITE_ENABLE_STAT4`), nach `V3__host_suffix.sql` und `ANALYZE`:
//!
//! | Abfrage | Release | Debug |
//! |---|---|---|
//! | `host:github.com` (trifft ein Drittel) | 3,5 ms | 45,3 ms |
//! | `host:nirgends.example` (trifft nichts) | 0,33 ms | 1,1 ms |
//! | Sortierung nach Host | 2,2 ms | 34,0 ms |
//! | Sortierung nach Dauer | 2,3 ms | 26,3 ms |
//! | Sortierung nach Größe | 2,3 ms | 29,8 ms |
//!
//! Die Pläne dahinter (der Lauf gibt sie mit aus):
//!
//! ```text
//! Zählung host:github.com  SEARCH flows USING COVERING INDEX flows_host_rev (host_rev>? AND host_rev<?)
//! Seite   host:github.com  SCAN flows USING INDEX flows_ts
//! Seite   seltener Host    SEARCH flows USING COVERING INDEX flows_host_rev (host_rev>? AND host_rev<?)
//! Seite   sort=size        SCAN flows USING INDEX flows_sort_size
//! ```
//!
//! Davor, mit dem Suffix-`LIKE`, `flows_ts(ts DESC, id ASC)` und ohne
//! Sortier-Indizes:
//!
//! ```text
//! Zählung host:github.com  SCAN flows USING COVERING INDEX flows_host
//! Seite   host:github.com  SCAN flows USING INDEX flows_ts
//!                          USE TEMP B-TREE FOR LAST TERM OF ORDER BY
//! Seite   sort=size        SCAN flows + USE TEMP B-TREE FOR ORDER BY
//! ```
//!
//! In Zahlen: 11,1 ms (Release) für den häufigen Host; für den seltenen lief
//! die Tabelle zweimal vollständig durch, und jede Sortierung außer der nach
//! Zeit kostete 202 bis 241 ms je Seite. Der Unterschied Debug zu Release ist kein Rust-Code, sondern
//! `SQLite` selbst: `libsqlite3-sys` wird im Debug-Profil mit `-O0` übersetzt.
//! Der Grenzwert richtet sich deshalb nach dem Profil; verbindlich ist der des
//! Release-Profils, mit dem der Daemon ausgeliefert wird.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant, SystemTime};

use humanitl_core::SessionId;
use humanitl_recorder::{FlowQuery, Recorder, RecorderSettings, SessionMeta, SortKey, millis};

/// So viele Zeilen stehen in der Tabelle.
const ROWS: usize = 100_000;

/// So lange darf die Abfrage im Release-Profil höchstens dauern.
const BUDGET: Duration = Duration::from_millis(50);

/// So lange darf sie im Debug-Profil dauern, in dem `SQLite` mit `-O0` liegt.
const DEBUG_BUDGET: Duration = Duration::from_millis(200);

#[tokio::test]
#[ignore = "builds 100k rows; run with --ignored, see the module comment"]
async fn list_flows_with_a_host_filter_over_100k_rows_stays_under_50ms() {
    let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
    let db = dir.path().join("humanitl.db");
    let blobs = dir.path().join("blobs");

    let session = SessionId::new();
    let started = SystemTime::now() - Duration::from_secs(3_600);
    {
        let recorder = Recorder::open(&db, &blobs, RecorderSettings::default())
            .unwrap_or_else(|err| panic!("{err}"));
        recorder.start_session(&SessionMeta {
            id: session,
            started_at: started,
            sandbox_profile: "default".to_owned(),
            llm_endpoint: None,
            work_dir: "/home/x/projekt".to_owned(),
            agent: "opencode".to_owned(),
        });
        recorder.flush().await;
    }

    let filled = Instant::now();
    fill(&db, session, millis(started));
    let filling = filled.elapsed();

    let recorder = Recorder::open(&db, &blobs, RecorderSettings::default())
        .unwrap_or_else(|err| panic!("{err}"));
    // Erst mit Statistiken unterscheidet der Planer den häufigen Host vom
    // seltenen; ohne sie ist einer der beiden Fälle um ein Vielfaches
    // langsamer. Im Betrieb erhebt sie jeder Aufräumlauf.
    recorder
        .analyze()
        .await
        .unwrap_or_else(|err| panic!("{err}"));

    let common = FlowQuery::new("host:github.com");
    let rare = FlowQuery::new("host:nirgends.example");

    // Einmal warmlaufen, damit nicht das erste Öffnen der Verbindung gemessen wird.
    let warm = recorder
        .list_flows(&common)
        .await
        .unwrap_or_else(|err| panic!("{err}"));
    assert_eq!(warm.rows.len(), 200, "the default page is 200 rows");

    let (best, rows) = measure(&recorder, &common).await;
    assert_eq!(rows, 200);

    // Der feindliche Fall: ein Host, den es in einer langen History nicht gibt.
    // Vor V3 lief dafür die ganze Tabelle durch, für die Zählung und für die
    // Seite je einmal.
    let (best_rare, rare_rows) = measure(&recorder, &rare).await;
    assert_eq!(rare_rows, 0);

    // Sortieren nach Host, Dauer und Größe lief vor `V3__host_suffix.sql` über
    // die ganze Tabelle plus eine vollständige Sortierung.
    let mut by_sort = Vec::new();
    for sort in [SortKey::Host, SortKey::Duration, SortKey::Size] {
        let query = FlowQuery {
            sort,
            ..FlowQuery::default()
        };
        let (best_sort, sorted_rows) = measure(&recorder, &query).await;
        assert_eq!(sorted_rows, 200);
        by_sort.push((sort, best_sort));
    }

    let budget = if cfg!(debug_assertions) {
        DEBUG_BUDGET
    } else {
        BUDGET
    };
    println!(
        "filled {ROWS} rows in {filling:?}; host:github.com took {best:?}, \
         host:nirgends.example took {best_rare:?}"
    );
    for (label, sql, params) in plan_probes() {
        println!(
            "   plan {label}: {}",
            plan_of(&db, &sql, &params).join(" | ")
        );
    }
    for (sort, elapsed) in &by_sort {
        println!("   sort by {} took {elapsed:?}", sort.as_str());
        assert!(
            *elapsed < budget,
            "sorting by {} took {elapsed:?}, the budget is {budget:?}",
            sort.as_str()
        );
    }
    assert!(
        best < budget,
        "list_flows over {ROWS} rows took {best:?}, the budget is {budget:?}"
    );
    assert!(
        best_rare < budget,
        "the rare host took {best_rare:?}, the budget is {budget:?}"
    );
}

/// Die Abfragen, deren Plan der Lauf mit ausgibt.
///
/// Das Akzeptanzkriterium nennt eine Zeit; der Plan sagt, warum sie so ist, und
/// ob sie mit wachsender History so bleibt.
fn plan_probes() -> Vec<(String, String, Vec<String>)> {
    let (low, high) = humanitl_recorder::suffix_range("github.com");
    let (rare_low, rare_high) = humanitl_recorder::suffix_range("nirgends.example");
    vec![
        (
            "count host:github.com".to_owned(),
            "SELECT COUNT(*) FROM (SELECT 1 FROM flows WHERE (host_rev >= ? AND host_rev < ?) \
             LIMIT 10001)"
                .to_owned(),
            vec![low.clone(), high.clone()],
        ),
        (
            "page host:github.com".to_owned(),
            "SELECT id FROM flows WHERE (host_rev >= ? AND host_rev < ?) \
             ORDER BY ts DESC, id DESC LIMIT 200"
                .to_owned(),
            vec![low, high],
        ),
        (
            "page host:nirgends.example".to_owned(),
            "SELECT id FROM flows WHERE (host_rev >= ? AND host_rev < ?) \
             ORDER BY ts DESC, id DESC LIMIT 200"
                .to_owned(),
            vec![rare_low, rare_high],
        ),
        (
            "page sort=size".to_owned(),
            "SELECT id FROM flows WHERE 1 ORDER BY (request_size + COALESCE(response_size, 0)) \
             DESC, ts DESC, id DESC LIMIT 200"
                .to_owned(),
            Vec::new(),
        ),
    ]
}

/// Der Plan zu einer Abfrage, Zeile für Zeile.
fn plan_of(db: &std::path::Path, sql: &str, params: &[String]) -> Vec<String> {
    let conn = rusqlite::Connection::open(db).unwrap_or_else(|err| panic!("{err}"));
    let mut statement = conn
        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
        .unwrap_or_else(|err| panic!("{err}"));
    let rows = statement
        .query_map(rusqlite::params_from_iter(params.iter()), |row| {
            row.get::<_, String>(3)
        })
        .unwrap_or_else(|err| panic!("{err}"));
    rows.map(Result::unwrap_or_default).collect()
}

/// Die schnellste von fünf Läufen einer Abfrage, samt Zeilenzahl.
async fn measure(recorder: &Recorder, query: &FlowQuery) -> (Duration, usize) {
    let mut best = Duration::MAX;
    let mut rows = 0;
    for _round in 0..5 {
        let started = Instant::now();
        let page = recorder
            .list_flows(query)
            .await
            .unwrap_or_else(|err| panic!("{err}"));
        best = best.min(started.elapsed());
        rows = page.rows.len();
    }
    (best, rows)
}

/// Schreibt [`ROWS`] Zeilen in einer Anweisung.
///
/// Die Zeilen entstehen in `SQLite` selbst (rekursives `CTE`), nicht in einer
/// Schleife in Rust: das Befüllen ist Vorbereitung, nicht Gegenstand der
/// Messung, und soll die Laufzeit der Testreihe nicht bestimmen.
///
/// Ein Drittel der Zeilen zeigt auf `api.github.com`, der Rest auf Hosts, die
/// der Filter nicht treffen darf (darunter `evil-github.com`).
fn fill(db: &std::path::Path, session: SessionId, base_ms: i64) {
    let conn = rusqlite::Connection::open(db).unwrap_or_else(|err| panic!("{err}"));
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=OFF;")
        .unwrap_or_else(|err| panic!("{err}"));
    conn.execute(
        "WITH RECURSIVE counter(n) AS (\
             SELECT 1 UNION ALL SELECT n + 1 FROM counter WHERE n < ?3\
         ) \
         INSERT INTO flows (id, session_id, seq, ts, method, scheme, host, host_display, \
             host_rev, port, path, state, request_size) \
         SELECT lower(hex(randomblob(16))), ?1, n, ?2 + n, 'GET', 'https', \
             CASE n % 3 WHEN 0 THEN 'api.github.com' \
                        WHEN 1 THEN 'evil-github.com' \
                        ELSE 'registry.npmjs.org' END, \
             CASE n % 3 WHEN 0 THEN 'api.github.com' \
                        WHEN 1 THEN 'evil-github.com' \
                        ELSE 'registry.npmjs.org' END, \
             CASE n % 3 WHEN 0 THEN 'com.github.api.' \
                        WHEN 1 THEN 'com.evil-github.' \
                        ELSE 'org.npmjs.registry.' END, \
             443, '/user', 'recorded', 0 \
         FROM counter",
        rusqlite::params![
            session.to_string(),
            base_ms,
            i64::try_from(ROWS).unwrap_or(i64::MAX)
        ],
    )
    .unwrap_or_else(|err| panic!("{err}"));
}
