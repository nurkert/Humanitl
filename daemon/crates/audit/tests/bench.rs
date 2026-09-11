//! Die Zusage „`verify` über 100 000 Records dauert unter 5 s" (HUM-050).
//!
//! Im normalen Lauf übersprungen; gemessen mit
//!
//! ```sh
//! cargo test -p humanitl-audit --release --test bench -- --ignored --nocapture
//! ```

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::{BufWriter, Write as _};
use std::time::{Duration, Instant};

use humanitl_audit::kinds::FlowReceived;
use humanitl_audit::{Anchor, AuditVerifier, GENESIS_PREV, RecordBody, RecordKind};
use humanitl_core::{Authority, BodyRef, FlowId, HostName, HttpRequest, Method, Scheme};

const RECORDS: u64 = 100_000;
const BUDGET: Duration = Duration::from_secs(5);

#[test]
#[ignore = "bench: builds and verifies 100 000 records"]
fn verify_100k_records_under_5s() {
    let key = [3_u8; 32];
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("audit.jsonl");
    let mut out = BufWriter::new(std::fs::File::create(&path).unwrap());
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
        let record = RecordBody {
            seq,
            ts: "2026-09-11T10:00:00.000000Z".to_owned(),
            session: "0190d8a4-8f9c-7c3e-9a1b-3c4d5e6f7a8b".to_owned(),
            kind: "flow.received".to_owned(),
            data,
            prev: prev.clone(),
        }
        .seal(&key)
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
    drop(out);

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
