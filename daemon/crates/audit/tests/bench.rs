//! Die Zusage „`verify` über 100 000 Records dauert unter 5 s" (HUM-050) und
//! die Dauer einer Seite der Audit-Tabelle über ebenso viele Records (HUM-163).
//!
//! Im normalen Lauf übersprungen; gemessen mit
//!
//! ```sh
//! cargo test -p humanitl-audit --release --test bench -- --ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufWriter, Write as _};
use std::path::Path;
use std::time::{Duration, Instant};

use humanitl_audit::kinds::FlowReceived;
use humanitl_audit::query::{self, QueryFilter};
use humanitl_audit::{Anchor, AuditVerifier, GENESIS_PREV, RecordBody, RecordKind};
use humanitl_core::{Authority, BodyRef, FlowId, HostName, HttpRequest, Method, Scheme};

const RECORDS: u64 = 100_000;
const BUDGET: Duration = Duration::from_secs(5);

/// Die Sitzung fast aller Records.
const SESSION: &str = "0190d8a4-8f9c-7c3e-9a1b-3c4d5e6f7a8b";
/// Die Sitzung jedes hundertsten Records: ein Filter, der selten trifft.
const RARE_SESSION: &str = "0190d8a4-8f9c-7c3e-9a1b-000000000100";

/// Schreibt `RECORDS` versiegelte Records nach `path` und liefert einen Anker
/// je hundert Records.
fn write_chain(path: &Path, key: &[u8; 32]) -> Vec<Anchor> {
    let mut out = BufWriter::new(std::fs::File::create(path).unwrap());
    let mut anchors = Vec::new();
    let mut prev = GENESIS_PREV.to_owned();
    let mut request = HttpRequest::new(
        Method::POST,
        Scheme::Https,
        Authority::with_scheme(HostName::Dns("api.example.com".to_owned()), Scheme::Https),
        "/v1/chat/completions?stream=true",
    );
    request.body = BodyRef::from_bytes(bytes::Bytes::from_static(&[b'x'; 2048]));
    for seq in 1..=RECORDS {
        let data = RecordKind::FlowReceived(FlowReceived::new(FlowId::new(), &request, &[])).data();
        let session = if seq % 100 == 0 {
            RARE_SESSION
        } else {
            SESSION
        };
        let record = RecordBody {
            seq,
            ts: "2026-09-11T10:00:00.000000Z".to_owned(),
            session: session.to_owned(),
            kind: "flow.received".to_owned(),
            data,
            prev: prev.clone(),
        }
        .seal(key)
        .unwrap();
        out.write_all(&record.to_line().unwrap()).unwrap();
        out.write_all(b"\n").unwrap();
        if seq % 100 == 0 {
            anchors.push(Anchor {
                seq,
                hash: record.hash.clone(),
                ts: record.body.ts.clone(),
            });
        }
        prev = record.hash;
    }
    out.flush().unwrap();
    anchors
}

#[test]
#[ignore = "bench: builds and verifies 100 000 records"]
fn verify_100k_records_under_5s() {
    let key = [3_u8; 32];
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let anchors = write_chain(&path, &key);

    let started = Instant::now();
    let report = AuditVerifier::verify(&path, Some(&key), &anchors).unwrap();
    let took = started.elapsed();
    println!(
        "verify: {RECORDS} records, {} bytes, {took:?}",
        std::fs::metadata(&path).unwrap().len()
    );
    assert!(report.is_ok(), "{report:?}");
    assert_eq!(report.records, RECORDS);
    assert!(
        took < BUDGET,
        "verify took {took:?}, the promise is {BUDGET:?}"
    );
}

/// So lange darf eine Seite über 100 000 Records dauern.
const PAGE_BUDGET: Duration = Duration::from_millis(200);

/// Misst eine Seite von 200 Records ohne Filter, die zweite Seite über den
/// Cursor, eine Seite mit einem Filter, der jeden Record trifft, und eine mit
/// einem Filter, der jeden hundertsten trifft. Die Messung vor und nach
/// HUM-163 steht in `backlog/sprint-4.md`.
#[test]
#[ignore = "bench: builds 100 000 records and reads pages of them"]
fn a_page_over_100k_records_is_fast() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    write_chain(&path, &[3_u8; 32]);
    let bytes = std::fs::metadata(&path).unwrap().len();
    let open = QueryFilter::default();

    let (first, took) = timed(|| query::query(&path, &open, 0, None, None).unwrap());
    println!("page without filter: {RECORDS} records, {bytes} bytes, {took:?}");
    assert_eq!(first.entries.len(), query::DEFAULT_PAGE);
    assert_eq!(first.entries[0].record.body.seq, RECORDS);
    assert_eq!(first.total, RECORDS);

    let (second, took_second) =
        timed(|| query::query(&path, &open, 0, first.next_before, None).unwrap());
    println!("second page without filter: {took_second:?}");
    assert_eq!(second.entries[0].record.body.seq, RECORDS - 200);

    let every = QueryFilter {
        kind_prefix: "flow.".to_owned(),
        ..QueryFilter::default()
    };
    let (full, took_every) = timed(|| query::query(&path, &every, 0, None, None).unwrap());
    println!("page with a filter that matches every record: {took_every:?}");
    assert_eq!(
        full, first,
        "a filter that matches everything changes nothing"
    );

    let rare = QueryFilter {
        session: RARE_SESSION.to_owned(),
        ..QueryFilter::default()
    };
    let (filtered, took_rare) = timed(|| query::query(&path, &rare, 0, None, None).unwrap());
    println!("page with a filter that matches 1 in 100: {took_rare:?}");
    assert_eq!(filtered.total, RECORDS / 100);
    assert_eq!(filtered.entries.len(), query::DEFAULT_PAGE);

    for (page, took) in [
        ("first", took),
        ("second", took_second),
        ("every", took_every),
        ("rare", took_rare),
    ] {
        assert!(
            took < PAGE_BUDGET,
            "the {page} page took {took:?}, the budget is {PAGE_BUDGET:?}"
        );
    }
}

/// Führt `run` aus und nennt, wie lange es gedauert hat.
fn timed<T>(run: impl FnOnce() -> T) -> (T, Duration) {
    let started = Instant::now();
    let value = run();
    (value, started.elapsed())
}
