//! Die Manipulationen aus `backlog/sprint-4.md` HUM-050, jede an einer echten
//! Kette mit echten Ankern in `SQLite`.
//!
//! Die Kette entsteht wie im Daemon: ein Schreiber mit `anchor_every = 5`,
//! dessen Anker in `audit_anchors` landen. Acht Records ergeben zehn Zeilen,
//! die fünfte und die zehnte sind Anker — das ist die Datei „mit 10 Records
//! und Ankern bei 5 und 10" der Spezifikation.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use humanitl_audit::{
    AuditVerifier, BreakReason, ExportFormat, RecordBody, TimeRange, VerifyReport, VerifyStatus,
    VerifyWarning, export,
};
use humanitl_core::diagnostics::codes::AUDIT_001;

use common::{Chain, KEY};

/// Zehn Zeilen, Anker bei 5 und 10.
fn ten() -> Chain {
    let chain = Chain::new();
    chain.write(5, 8);
    assert_eq!(chain.lines().len(), 10);
    let anchored: Vec<u64> = chain.anchors().iter().map(|anchor| anchor.seq).collect();
    assert_eq!(anchored, [5, 10]);
    assert!(chain.verify().is_ok(), "the untouched chain holds");
    chain
}

fn broken(first_bad_seq: u64, reason: BreakReason) -> VerifyStatus {
    VerifyStatus::Broken {
        first_bad_seq,
        reason,
    }
}

#[test]
fn modify_field_detected() {
    let chain = ten();
    let mut lines = chain.lines();
    assert!(lines[3].contains("\"decision\":\"block\""), "{}", lines[3]);
    lines[3] = lines[3].replacen("\"decision\":\"block\"", "\"decision\":\"allow\"", 1);
    chain.set_lines(&lines);
    assert_eq!(chain.verify().status, broken(4, BreakReason::HashMismatch));
}

#[test]
fn delete_middle_detected() {
    let chain = ten();
    let mut lines = chain.lines();
    lines.remove(3);
    chain.set_lines(&lines);
    // Record 5 folgt auf 3: Er ist der erste, der nicht passt.
    assert_eq!(chain.verify().status, broken(5, BreakReason::SeqGap));
}

/// Die Spezifikation nennt hier `Broken{6, SeqGap}`, beim Löschen aber
/// `Broken{5, SeqGap}` für „5 folgt auf 3" — die erwartete Nummer im einen,
/// die gefundene im anderen Fall. Beides zugleich geht nicht. Die Prüfung
/// nennt durchgehend die Nummer, die im ersten nicht passenden Record steht
/// (Modulkommentar von `verify`); an Zeile 6 steht nach dem Tausch Record 7.
#[test]
fn reorder_detected() {
    let chain = ten();
    let mut lines = chain.lines();
    lines.swap(5, 6);
    chain.set_lines(&lines);
    assert_eq!(chain.verify().status, broken(7, BreakReason::SeqGap));
}

#[test]
fn recompute_without_key_detected() {
    let chain = ten();
    let mut records = chain.records();
    records[3].body.data["decision"] = "allow".into();
    // Hash und alle folgenden `prev`/`hash` korrekt nachgerechnet, der MAC mit
    // einem anderen Schlüssel: Wer den echten nicht hat, kann nicht anders.
    let mut prev = records[2].hash.clone();
    for record in records.iter_mut().skip(3) {
        let body = RecordBody {
            prev: prev.clone(),
            ..record.body.clone()
        };
        *record = body.seal(&[9; 32]).unwrap();
        prev.clone_from(&record.hash);
    }
    let lines: Vec<String> = records
        .iter()
        .map(|record| String::from_utf8(record.to_line().unwrap()).unwrap())
        .collect();
    chain.set_lines(&lines);
    assert_eq!(chain.verify().status, broken(4, BreakReason::MacMismatch));
}

#[test]
fn truncate_below_anchor_detected() {
    let chain = ten();
    let lines = chain.lines();
    chain.set_lines(&lines[..7]);
    assert_eq!(
        chain.verify().status,
        broken(7, BreakReason::TruncatedBelowAnchor { anchor_seq: 10 })
    );
}

/// Die dokumentierte Grenze, als Test: Was hinter dem letzten Anker steht,
/// kann am Ende abgeschnitten werden, ohne dass die Prüfung es merkt
/// (`docs/SECURITY.md`, „Was die Audit-Kette beweist", Punkt 3). Liegt der
/// Schnitt genau auf dem letzten Anker, bleibt nicht einmal eine Warnung.
#[test]
fn truncate_above_last_anchor_not_detected_documented() {
    let twelve = Chain::new();
    twelve.write(5, 10);
    assert_eq!(twelve.lines().len(), 12);
    let lines = twelve.lines();
    twelve.set_lines(&lines[..10]);
    let report = twelve.verify();
    assert_eq!(report.status, VerifyStatus::Ok);
    assert_eq!(report.warnings, vec![]);

    // Bleibt ein Record hinter dem Anker stehen, sagt es wenigstens die
    // Warnung: einer ist unverankert.
    let thirteen = Chain::new();
    thirteen.write(5, 11);
    assert_eq!(thirteen.lines().len(), 13);
    let lines = thirteen.lines();
    thirteen.set_lines(&lines[..11]);
    let report = thirteen.verify();
    assert_eq!(report.status, VerifyStatus::Ok);
    assert_eq!(
        report.warnings,
        vec![VerifyWarning::UnanchoredTail { records: 1 }]
    );
}

#[test]
fn non_canonical_line_detected() {
    let chain = ten();
    let mut lines = chain.lines();
    lines[1] = lines[1].replacen("{\"data\":", "{ \"data\":", 1);
    chain.set_lines(&lines);
    assert_eq!(
        chain.verify().status,
        broken(2, BreakReason::NonCanonicalLine)
    );
}

#[test]
fn anchor_tampered_detected() {
    let chain = ten();
    let conn = rusqlite::Connection::open(&chain.db).unwrap();
    let changed = conn
        .execute(
            "UPDATE audit_anchors SET hash = ?1 WHERE seq = 5",
            [&"f".repeat(64)],
        )
        .unwrap();
    assert_eq!(changed, 1);
    drop(conn);
    assert_eq!(
        chain.verify().status,
        broken(5, BreakReason::AnchorMismatch { anchor_seq: 5 })
    );
}

// ---------------------------------------------------------------------------
// Über die acht der Spezifikation hinaus: die übrigen Gründe und die Grenzen.
// ---------------------------------------------------------------------------

#[test]
fn a_changed_prev_resealed_with_the_key_is_a_prev_mismatch() {
    let chain = ten();
    let mut records = chain.records();
    let body = RecordBody {
        prev: records[1].hash.clone(),
        ..records[3].body.clone()
    };
    records[3] = body.seal(&KEY).unwrap();
    let lines: Vec<String> = records
        .iter()
        .map(|record| String::from_utf8(record.to_line().unwrap()).unwrap())
        .collect();
    chain.set_lines(&lines);
    assert_eq!(chain.verify().status, broken(4, BreakReason::PrevMismatch));
}

#[test]
fn a_torn_last_line_is_not_canonical() {
    let chain = ten();
    let mut bytes = std::fs::read(&chain.log).unwrap();
    bytes.extend_from_slice(b"{\"data\"");
    std::fs::write(&chain.log, bytes).unwrap();
    assert_eq!(
        chain.verify().status,
        broken(11, BreakReason::NonCanonicalLine)
    );
}

/// Ohne Schlüssel bleibt der MAC ungeprüft, und das steht als Warnung im
/// Bericht. Eine neu gebaute Kette fällt dann nur noch an den Ankern auf —
/// und nicht mehr, wenn auch die Anker neu geschrieben sind. Das ist die
/// Grenze „wer den Keyring hat, kann die Kette neu bauen", ohne Keyring.
#[test]
fn without_a_key_a_rebuilt_chain_is_caught_only_by_the_anchors() {
    let chain = ten();
    let mut records = chain.records();
    records[3].body.data["decision"] = "allow".into();
    let mut prev = records[2].hash.clone();
    for record in records.iter_mut().skip(3) {
        let body = RecordBody {
            prev: prev.clone(),
            ..record.body.clone()
        };
        *record = body.seal(&[9; 32]).unwrap();
        prev.clone_from(&record.hash);
    }
    let lines: Vec<String> = records
        .iter()
        .map(|record| String::from_utf8(record.to_line().unwrap()).unwrap())
        .collect();
    chain.set_lines(&lines);

    let report = AuditVerifier::verify(&chain.log, None, &chain.anchors()).unwrap();
    assert_eq!(
        report.status,
        broken(5, BreakReason::AnchorMismatch { anchor_seq: 5 })
    );
    assert_eq!(report.warnings, vec![VerifyWarning::NoHmacKey]);

    let rebuilt: Vec<humanitl_audit::Anchor> = [4_usize, 9]
        .iter()
        .map(|&index| humanitl_audit::Anchor {
            seq: records[index].body.seq,
            hash: records[index].hash.clone(),
            ts: records[index].body.ts.clone(),
        })
        .collect();
    let report: VerifyReport = AuditVerifier::verify(&chain.log, None, &rebuilt).unwrap();
    assert_eq!(report.status, VerifyStatus::Ok);
    assert_eq!(report.warnings, vec![VerifyWarning::NoHmacKey]);
    // Mit Schlüssel fällt derselbe Neubau sofort auf.
    let report = AuditVerifier::verify(&chain.log, Some(&KEY), &rebuilt).unwrap();
    assert_eq!(report.status, broken(4, BreakReason::MacMismatch));
}

#[test]
fn a_broken_report_becomes_audit_001() {
    let chain = ten();
    let lines = chain.lines();
    chain.set_lines(&lines[..7]);
    let report = chain.verify();
    let diagnostic = report.diagnostic(&chain.log).expect("a broken chain");
    assert_eq!(diagnostic.code.as_str(), "AUDIT_001");
    assert!(diagnostic.why.contains("seq 10"), "{}", diagnostic.why);
    assert!(ten().verify().diagnostic(&chain.log).is_none());
}

/// Bis zum gemeldeten Ende geprüft, zählen Anker dahinter nicht: Sie gehören
/// zu Records, die diese Prüfung noch nicht liest (HUM-156). Ohne Ende wären
/// sie ein Bruch.
#[test]
fn anchors_behind_the_reported_end_do_not_count() {
    let chain = ten();
    let mut anchors = chain.anchors();
    let end = chain.records().last().unwrap().body.seq;
    anchors.push(humanitl_audit::Anchor {
        seq: end + 1,
        hash: "0".repeat(64),
        ts: "2026-09-18T10:00:00.000000Z".to_owned(),
    });
    let until = AuditVerifier::verify_until(&chain.log, Some(&KEY), &anchors, Some(end)).unwrap();
    assert_eq!(until.status, VerifyStatus::Ok);
    let whole = AuditVerifier::verify(&chain.log, Some(&KEY), &anchors).unwrap();
    assert_eq!(
        whole.status,
        broken(
            end,
            BreakReason::TruncatedBelowAnchor {
                anchor_seq: end + 1
            }
        )
    );
}

#[test]
fn a_missing_log_with_anchors_is_truncated_to_nothing() {
    let chain = ten();
    std::fs::remove_file(&chain.log).unwrap();
    assert_eq!(
        chain.verify().status,
        broken(0, BreakReason::TruncatedBelowAnchor { anchor_seq: 5 })
    );
}

/// HUM-214: Der Export prüft die Kette nicht, er kopiert sie. Eine von `block`
/// auf `allow` geänderte Zeile geht Byte für Byte in den JSONL-Export, und
/// `verify` findet den Bruch dort an derselben Stelle wie im Log. Verweigerte
/// der Export eine gebrochene Kette, ließe sich ein manipuliertes Log nicht
/// mehr als Beleg übergeben (`docs/SECURITY.md`).
#[test]
fn a_tampered_chain_is_exported_unchanged_and_verify_finds_it() {
    let chain = ten();
    let mut lines = chain.lines();
    lines[3] = lines[3].replacen("\"decision\":\"block\"", "\"decision\":\"allow\"", 1);
    chain.set_lines(&lines);
    assert_eq!(chain.verify().status, broken(4, BreakReason::HashMismatch));

    let out = chain.dir.path().join("export.jsonl");
    let count = export::export(&chain.log, ExportFormat::Jsonl, &TimeRange::ALL, &out, None)
        .expect("the export copies a broken chain");
    assert_eq!(count, 10);
    assert_eq!(
        std::fs::read(&out).unwrap(),
        std::fs::read(&chain.log).unwrap(),
        "the export is the chain byte for byte"
    );
    let report = AuditVerifier::verify(&out, Some(&KEY), &chain.anchors()).unwrap();
    assert_eq!(report.status, broken(4, BreakReason::HashMismatch));
}

/// HUM-214: Was der Export verweigert, ist eine Zeile, die kein Record ist;
/// dann schreibt er nichts.
#[test]
fn a_line_that_is_no_record_stops_the_export_with_audit_001() {
    let chain = ten();
    let mut lines = chain.lines();
    lines[3] = "not a record".to_owned();
    chain.set_lines(&lines);

    let out = chain.dir.path().join("export.jsonl");
    let error = export::export(&chain.log, ExportFormat::Jsonl, &TimeRange::ALL, &out, None)
        .expect_err("a line that is no record is refused");
    assert_eq!(error.code, AUDIT_001);
    assert!(!out.exists(), "nothing is written");
}

/// HUM-214: Eine leere Zeile und eine aus Leerraum sind für `verify` ein
/// Bruch. Der Export ließe sie nicht still weg, sonst wäre er nicht mehr Byte
/// für Byte die Kette und der Bruch aus dem Beleg verschwunden.
#[test]
fn a_blank_line_stops_the_export_with_audit_001() {
    for blank in ["", " \t "] {
        let chain = ten();
        let mut lines = chain.lines();
        lines.insert(4, blank.to_owned());
        chain.set_lines(&lines);
        assert_eq!(
            chain.verify().status,
            broken(5, BreakReason::NonCanonicalLine),
            "{blank:?}"
        );

        let out = chain.dir.path().join("export.jsonl");
        let error = export::export(&chain.log, ExportFormat::Jsonl, &TimeRange::ALL, &out, None)
            .expect_err("a blank line is no record");
        assert_eq!(error.code, AUDIT_001, "{blank:?}");
        assert!(!out.exists(), "nothing is written for {blank:?}");
    }
}
