//! Der Schreiber der Audit-Kette (HUM-050): Anfang, Wiederaufnahme, Anker,
//! Ende, die Gründe, aus denen er nicht anhängt, und die Lücke, hinter der
//! er weiterläuft.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::fs;

use chrono::{TimeZone as _, Utc};
use humanitl_audit::kinds::{DaemonStarted, DaemonStopped};
use humanitl_audit::{
    AuditKey, BreakReason, GENESIS_PREV, KeyOrigin, RecordBody, RecordKind, VerifyStatus,
    VerifyWarning, format_ts,
};
use humanitl_core::Severity;

use common::{Chain, decided};

#[test]
fn genesis_prev_is_zeros() {
    let chain = Chain::new();
    chain.write(100, 2);
    let records = chain.records();
    assert_eq!(records[0].body.seq, 1);
    assert_eq!(records[0].body.prev, GENESIS_PREV);
    assert_eq!(GENESIS_PREV, "0".repeat(64));
    assert_eq!(records[0].body.session, "-");
    assert_eq!(records[1].body.prev, records[0].hash);
}

/// Hash und MAC gegen Werte, die nicht aus diesem Code stammen: Sie sind mit
/// Pythons `hashlib` und `hmac` aus der von Hand geschriebenen kanonischen
/// Form berechnet. Wer die Kanonisierung, die Feldauswahl des Hashes, das
/// Zeitformat oder den MAC ändert, macht diesen Test rot, und genau das soll
/// er: Jede solche Änderung macht jedes vorhandene Log unprüfbar.
#[test]
fn hash_golden_vector() {
    let key: [u8; 32] = core::array::from_fn(|index| u8::try_from(index).unwrap());
    let ts = Utc
        .with_ymd_and_hms(2026, 9, 2, 10, 0, 0)
        .unwrap()
        .checked_add_signed(chrono::Duration::microseconds(123_456))
        .unwrap();
    assert_eq!(format_ts(ts), "2026-09-02T10:00:00.123456Z");

    let first = RecordBody {
        seq: 1,
        ts: format_ts(ts),
        session: "-".to_owned(),
        kind: "daemon.started".to_owned(),
        data: RecordKind::DaemonStarted(DaemonStarted {
            version: "0.0.0".to_owned(),
            proto_version: "1.7".to_owned(),
            key_origin: KeyOrigin::File,
        })
        .data(),
        prev: GENESIS_PREV.to_owned(),
    }
    .seal(&key)
    .unwrap();
    assert_eq!(
        first.hash,
        "dbef20a62b10bde750fd4c646de7cb109b144ef35d97568e317db75b49a91faa"
    );
    assert_eq!(
        first.mac,
        "642c8b78464d9c53b733922005475ab425a1f02739339e373eaf35d8167a158d"
    );

    // Der zweite Record hängt am ersten und trägt, was die Kanonisierung am
    // ehesten falsch macht: Nicht-ASCII, ein Steuerzeichen und `/`.
    let second = RecordBody {
        seq: 2,
        ts: "2026-09-02T10:00:01.000000Z".to_owned(),
        session: "-".to_owned(),
        kind: "daemon.stopped".to_owned(),
        data: RecordKind::DaemonStopped(DaemonStopped {
            reason: "Stopp ü\n/x".to_owned(),
        })
        .data(),
        prev: first.hash.clone(),
    }
    .seal(&key)
    .unwrap();
    assert_eq!(
        second.hash,
        "b016ed2577512e80c2f35302123a80cc3ab607f29164b9bc2637197cafa69b02"
    );
    assert_eq!(
        second.mac,
        "51762d717f5a835f22719c4b928ef7c4e53fa08ae78bd0f3db70af7ee48a3033"
    );
    let line = String::from_utf8(second.to_line().unwrap()).unwrap();
    assert!(line.contains("\"reason\":\"Stopp ü\\u000a/x\""), "{line}");
}

#[test]
fn writer_resumes_from_existing_file() {
    let chain = Chain::new();
    chain.write(100, 3);
    let third = chain.records()[2].clone();

    let writer = chain.open(100);
    assert_eq!(writer.resumed().seq, 3);
    assert_eq!(writer.resumed().hash, third.hash);
    writer.handle().record(None, decided(3));
    writer.handle().sync().unwrap();
    drop(writer);

    let records = chain.records();
    assert_eq!(records.len(), 4);
    assert_eq!(records[3].body.seq, 4);
    assert_eq!(records[3].body.prev, third.hash);
    assert!(chain.verify().is_ok());
}

#[test]
fn anchor_written_every_n() {
    let chain = Chain::new();
    chain.write(3, 5);

    let records = chain.records();
    assert_eq!(records.len(), 7, "five records and two anchors");
    let kinds: Vec<&str> = records
        .iter()
        .map(|record| record.body.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        [
            "flow.decided",
            "flow.decided",
            "audit.anchor",
            "flow.decided",
            "flow.decided",
            "audit.anchor",
            "flow.decided"
        ]
    );
    // In der Datei nennt der Anker seinen Vorgänger ...
    assert_eq!(records[2].body.data["anchored_seq"], 2);
    assert_eq!(
        records[2].body.data["anchored_hash"],
        records[1].hash.as_str()
    );
    assert_eq!(records[5].body.data["anchored_seq"], 5);
    // ... und in SQLite steht er selbst, mit Nummer und Hash.
    let anchors: Vec<(u64, String)> = chain
        .anchors()
        .into_iter()
        .map(|anchor| (anchor.seq, anchor.hash))
        .collect();
    assert_eq!(
        anchors,
        vec![(3, records[2].hash.clone()), (6, records[5].hash.clone())]
    );

    let report = chain.verify();
    assert_eq!(report.status, VerifyStatus::Ok);
    assert_eq!(
        report.warnings,
        vec![VerifyWarning::UnanchoredTail { records: 1 }]
    );
}

#[test]
fn anchor_on_stop() {
    let chain = Chain::new();
    let writer = chain.open(100);
    writer.handle().record(None, decided(0));
    writer.handle().record(None, decided(1));
    let head = writer.stop("test").expect("the writer was running");

    let records = chain.records();
    let kinds: Vec<&str> = records
        .iter()
        .map(|record| record.body.kind.as_str())
        .collect();
    assert_eq!(
        kinds,
        [
            "flow.decided",
            "flow.decided",
            "daemon.stopped",
            "audit.anchor"
        ]
    );
    assert_eq!(records[2].body.data["reason"], "test");
    assert_eq!(head.seq, 4);
    assert_eq!(head.hash, records[3].hash);
    let anchors = chain.anchors();
    assert_eq!(anchors.len(), 1);
    assert_eq!((anchors[0].seq, &anchors[0].hash), (4, &records[3].hash));

    // Ein geordnet beendetes Log hat kein unverankertes Ende.
    let report = chain.verify();
    assert_eq!(report.status, VerifyStatus::Ok);
    assert!(report.warnings.is_empty(), "{:?}", report.warnings);
}

#[test]
fn a_stop_right_after_an_anchor_writes_no_second_one() {
    let chain = Chain::new();
    let writer = chain.open(3);
    // `daemon.stopped` wird Record 2, der Anker danach Record 3 — genau der,
    // den `anchor_every = 3` ohnehin verlangt.
    writer.handle().record(None, decided(0));
    let head = writer.stop("test").unwrap();
    assert_eq!(head.seq, 3);
    assert_eq!(chain.anchors().len(), 1);
}

#[test]
fn a_torn_last_line_is_set_aside_and_the_chain_continues() {
    let chain = Chain::new();
    chain.write(100, 3);
    let third = chain.records()[2].hash.clone();
    let mut bytes = fs::read(&chain.log).unwrap();
    bytes.extend_from_slice(b"{\"data\":{\"flow\":\"01");
    fs::write(&chain.log, &bytes).unwrap();

    let (writer, notes) = chain.try_open(100).unwrap();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].code.as_str(), "AUDIT_002");
    assert_eq!(notes[0].severity, Severity::Warning);
    let aside: Vec<_> = fs::read_dir(chain.log.parent().unwrap())
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.to_string_lossy().contains("audit.jsonl.corrupt-"))
        .collect();
    assert_eq!(aside.len(), 1, "{aside:?}");
    assert_eq!(fs::read(&aside[0]).unwrap(), b"{\"data\":{\"flow\":\"01");
    assert!(notes[0].why.contains(&aside[0].display().to_string()));

    writer.handle().record(None, decided(3));
    writer.handle().sync().unwrap();
    drop(writer);
    let records = chain.records();
    assert_eq!(records.len(), 4);
    assert_eq!(records[3].body.seq, 4, "no gap");
    assert_eq!(records[3].body.prev, third);
    assert!(chain.verify().is_ok());
}

#[test]
fn a_second_writer_on_the_same_log_is_refused() {
    let chain = Chain::new();
    let first = chain.open(100);
    let error = chain.try_open(100).unwrap_err();
    assert_eq!(error.code.as_str(), "AUDIT_004");
    assert_eq!(error.severity, Severity::Blocking);
    drop(first);
    // Ist der erste weg, geht es.
    drop(chain.open(100));
}

#[test]
fn a_tampered_end_is_not_appended_to() {
    let chain = Chain::new();
    chain.write(100, 3);
    let mut lines = chain.lines();
    lines[2] = lines[2].replacen("\"decision\":\"allow\"", "\"decision\":\"block\"", 1);
    chain.set_lines(&lines);

    let error = chain.try_open(100).unwrap_err();
    assert_eq!(error.code.as_str(), "AUDIT_001");
    assert!(error.why.contains("hash"), "{}", error.why);
    let Some(humanitl_core::FixAction::CopyCommand(command)) = error.fix else {
        panic!("a command that sets the file aside: {:?}", error.fix);
    };
    assert!(command.starts_with("mv "), "{command}");
}

/// Ein Log, das unter einem Anker endet, sperrt den Daemon nicht aus: Die
/// Kette läuft hinter dem letzten Anker weiter, und die Lücke bleibt ein Bruch.
#[test]
fn a_log_cut_below_an_anchor_continues_after_the_last_anchor() {
    let chain = Chain::new();
    // Records 1 bis 7, Anker bei 3 und 6.
    chain.write(3, 5);
    let last = chain
        .anchors()
        .into_iter()
        .max_by_key(|anchor| anchor.seq)
        .unwrap();
    let lines = chain.lines();
    chain.set_lines(&lines[..4]);

    let (writer, notes) = chain.try_open(3).unwrap();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert_eq!(notes[0].code.as_str(), "AUDIT_007");
    assert_eq!(notes[0].severity, Severity::Warning);
    assert!(notes[0].why.contains("ends at seq 4"), "{}", notes[0].why);
    assert!(
        notes[0].why.contains(&format!("names seq {}", last.seq)),
        "{}",
        notes[0].why
    );
    assert_eq!(writer.resumed().seq, last.seq);
    drop(writer);

    let records = chain.records();
    assert_eq!(records.len(), 5, "the four kept records and the marker");
    let marker = &records[4];
    assert_eq!(marker.body.kind, "audit.resumed");
    assert_eq!(marker.body.seq, last.seq + 1);
    assert_eq!(marker.body.prev, last.hash);
    assert_eq!(marker.body.data["log_seq"], 4);
    assert_eq!(marker.body.data["anchor_seq"], last.seq);
    assert_eq!(
        chain.verify().status,
        VerifyStatus::Broken {
            first_bad_seq: last.seq + 1,
            reason: BreakReason::SeqGap,
        },
        "the gap stays a break"
    );
    assert!(
        chain.anchors().contains(&last),
        "the old anchors stay as evidence"
    );
}

/// Dasselbe für ein gelöschtes Log, wie es der `fix` von `AUDIT_001`
/// hinterlässt; der Start danach ist wieder ein gewöhnlicher.
#[test]
fn a_deleted_log_with_anchors_left_starts_after_the_last_anchor() {
    let chain = Chain::new();
    chain.write(3, 5);
    let last = chain
        .anchors()
        .into_iter()
        .max_by_key(|anchor| anchor.seq)
        .unwrap();
    fs::remove_file(&chain.log).unwrap();

    let (writer, notes) = chain.try_open(3).unwrap();
    let codes: Vec<&str> = notes.iter().map(|note| note.code.as_str()).collect();
    assert_eq!(codes, ["AUDIT_007"]);
    assert!(notes[0].why.contains("ends at seq 0"), "{}", notes[0].why);
    drop(writer);

    let records = chain.records();
    assert_eq!(records[0].body.kind, "audit.resumed");
    assert_eq!(
        (records[0].body.seq, &records[0].body.prev),
        (last.seq + 1, &last.hash)
    );
    assert_eq!(
        chain.verify().status,
        VerifyStatus::Broken {
            first_bad_seq: last.seq + 1,
            reason: BreakReason::SeqGap,
        }
    );

    // Das Ende liegt jetzt hinter jedem Anker, und neue Anker stehen neben
    // den alten, ohne mit ihnen um eine Nummer zu streiten.
    let writer = chain.open(3);
    assert_eq!(writer.resumed().seq, last.seq + 1);
    let head = writer.stop("done").unwrap();
    let anchors = chain.anchors();
    assert!(anchors.contains(&last), "{anchors:?}");
    assert!(
        anchors.iter().any(|anchor| anchor.seq == head.seq),
        "{anchors:?}"
    );
}

#[test]
fn another_key_is_not_appended_with() {
    let chain = Chain::new();
    chain.write(100, 2);
    let other = AuditKey::from_bytes([8; 32], KeyOrigin::File);
    let error = chain.try_open_with(&other, 100).unwrap_err();
    assert_eq!(error.code.as_str(), "AUDIT_001");
    assert!(error.why.contains("MAC"), "{}", error.why);
}
