//! Die Aufbewahrung der Kette (HUM-157), an einer echten Kette mit echten
//! Ankern in `SQLite`.
//!
//! Die Kette entsteht wie im Daemon. Zwei Schübe mit einer Pause dazwischen:
//! Der erste liegt vor der Grenze, der zweite danach. So hängt kein Test an
//! der Frage, ob zwei schnell geschriebene Records dieselbe Mikrosekunde
//! tragen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::{Duration, SystemTime};

use humanitl_audit::{
    Anchor, AuditRecord, AuditVerifier, BreakReason, VerifyStatus, VerifyWarning,
};
use humanitl_audit_log::{AuditWriter, VerifyFile as _};

use common::{Chain, KEY, decided};

/// Schreibt `users` Records und wartet, bis sie auf der Platte sind.
fn write(writer: &AuditWriter, users: usize) {
    for index in 0..users {
        writer.handle().record(None, decided(index));
    }
    writer.handle().sync().unwrap();
}

/// Eine Kette aus zwei Schüben: acht Records (zehn Zeilen, Anker bei 5 und
/// 10) vor der Grenze, drei danach. Zurück kommen Kette, offener Schreiber
/// und die Grenze.
fn two_batches() -> (Chain, AuditWriter, SystemTime) {
    let chain = Chain::new();
    let writer = chain.open(5);
    write(&writer, 8);
    std::thread::sleep(Duration::from_millis(5));
    let cutoff = SystemTime::now();
    std::thread::sleep(Duration::from_millis(5));
    write(&writer, 3);
    (chain, writer, cutoff)
}

/// Die Kette ohne Schreiber, wie `humanitl audit verify` sie sieht.
fn verify_with(chain: &Chain, anchors: &[Anchor]) -> humanitl_audit::VerifyReport {
    AuditVerifier::verify(&chain.log, Some(&KEY), anchors).unwrap()
}

#[test]
fn a_pruned_log_verifies_with_a_documented_gap() {
    let (chain, writer, cutoff) = two_batches();
    let anchors_before = chain.anchors();
    assert_eq!(chain.lines().len(), 13, "ten lines, then three records");

    let report = writer
        .handle()
        .prune(cutoff, chain.anchors())
        .unwrap()
        .expect("the first batch is older than the cutoff");
    assert_eq!(report.through_seq, 10, "the first batch ends on its anchor");
    assert_eq!(report.records, 10);

    // Der Anfang ist weg, der Rest steht wie zuvor, dahinter `audit.pruned`
    // und sein Anker.
    let records = chain.records();
    let seqs: Vec<u64> = records.iter().map(|record| record.body.seq).collect();
    assert_eq!(seqs, [11, 12, 13, 14, 15]);
    assert_eq!(records[3].body.kind, "audit.pruned");
    assert_eq!(records[3].body.data["through_seq"], 10);
    assert_eq!(records[3].body.data["records"], 10);
    assert_eq!(records[3].body.data["through_hash"], records[0].body.prev);
    assert_eq!(records[4].body.kind, "audit.anchor");

    // Kein Anker ging verloren; der Record des Laufs ist verankert.
    let anchors = chain.anchors();
    for anchor in &anchors_before {
        assert!(anchors.contains(anchor), "{anchor:?} was deleted");
    }
    assert_eq!(
        anchors.last().map(|anchor| anchor.seq),
        Some(15),
        "the run anchors its own record"
    );

    let verified = chain.verify();
    assert_eq!(verified.status, VerifyStatus::Ok, "{verified:?}");
    assert_eq!(verified.records, 5, "five records are there to hold");
    assert_eq!(
        verified.warnings,
        [VerifyWarning::Pruned { through_seq: 10 }],
        "the gap is documented, and nothing behind the last anchor"
    );
    assert_eq!(verified.head.as_ref().map(|head| head.seq), Some(15));

    // Der Schreiber hängt an die neue Datei an, und ein neuer Schreiber
    // setzt dort fort, ohne Befund.
    write(&writer, 2);
    drop(writer);
    assert_eq!(
        chain.records().len(),
        7,
        "the appended records are in the file"
    );
    let (reopened, notes) = chain.try_open(5).unwrap();
    assert!(notes.is_empty(), "{notes:?}");
    write(&reopened, 1);
    drop(reopened);
    let verified = chain.verify();
    assert!(verified.is_ok(), "{verified:?}");
    assert_eq!(verified.records, 8, "{verified:?}");
}

#[test]
fn a_gap_without_a_documenting_record_stays_a_break() {
    // Von Hand vorn gekürzt, genau auf dem Anker bei 5: Kein Record nennt
    // diesen Schnitt, also ist er eine Lücke wie vor HUM-157.
    let chain = Chain::new();
    chain.write(5, 8);
    let lines = chain.lines();
    chain.set_lines(&lines[5..]);
    let verified = chain.verify();
    assert_eq!(
        verified.status,
        VerifyStatus::Broken {
            first_bad_seq: 6,
            reason: BreakReason::SeqGap,
        },
        "{verified:?}"
    );
    assert_eq!(verified.records, 0, "nothing before the gap holds");
}

#[test]
fn cutting_further_than_the_record_says_is_a_break() {
    // Nach einem ehrlichen Lauf eine Zeile mehr vorn weg: Der Schnitt, den
    // `audit.pruned` nennt, ist nicht mehr der Anfang der Datei.
    let (chain, writer, cutoff) = two_batches();
    writer
        .handle()
        .prune(cutoff, chain.anchors())
        .unwrap()
        .unwrap();
    drop(writer);
    assert!(chain.verify().is_ok());

    let lines = chain.lines();
    chain.set_lines(&lines[1..]);
    let verified = chain.verify();
    assert_eq!(
        verified.status,
        VerifyStatus::Broken {
            first_bad_seq: 12,
            reason: BreakReason::SeqGap,
        },
        "{verified:?}"
    );
}

#[test]
fn a_broken_chain_is_not_pruned() {
    let (chain, writer, cutoff) = two_batches();
    let mut lines = chain.lines();
    lines[3] = lines[3].replacen("\"decision\":\"block\"", "\"decision\":\"allow\"", 1);
    chain.set_lines(&lines);
    let before = std::fs::read(&chain.log).unwrap();

    let refused = writer.handle().prune(cutoff, chain.anchors()).unwrap_err();
    assert_eq!(refused.code.as_str(), "AUDIT_001", "{refused:?}");
    assert!(
        refused.why.contains("deletes nothing"),
        "the finding says nothing went: {}",
        refused.why
    );
    assert!(refused.fix.is_some(), "the finding names the way out");
    drop(writer);
    assert_eq!(
        std::fs::read(&chain.log).unwrap(),
        before,
        "the evidence of the break stays byte for byte"
    );
}

#[test]
fn nothing_old_enough_changes_nothing() {
    let (chain, writer, _) = two_batches();
    let before = std::fs::read(&chain.log).unwrap();
    let long_ago = SystemTime::now() - Duration::from_secs(3_600);
    assert!(
        writer
            .handle()
            .prune(long_ago, chain.anchors())
            .unwrap()
            .is_none()
    );
    drop(writer);
    assert_eq!(std::fs::read(&chain.log).unwrap(), before);
}

#[test]
fn only_the_run_writes_audit_pruned() {
    // Ein `audit.pruned` über das Handle wäre ein Beleg für einen Schnitt, den
    // niemand gemacht hat.
    let chain = Chain::new();
    let writer = chain.open(5);
    writer.handle().record(
        None,
        humanitl_audit::RecordKind::AuditPruned(humanitl_audit::AuditPruned {
            through_seq: 4,
            through_hash: "0".repeat(64),
            records: 4,
            cutoff: Some("2026-01-01T00:00:00.000000Z".to_owned()),
            max_bytes: None,
        }),
    );
    writer.handle().sync().unwrap();
    drop(writer);
    assert!(
        chain.records().is_empty(),
        "the handle refused the record: {:?}",
        chain.lines()
    );
}

#[test]
fn an_anchor_on_the_cut_must_name_the_hash_the_log_names() {
    let (chain, writer, cutoff) = two_batches();
    writer
        .handle()
        .prune(cutoff, chain.anchors())
        .unwrap()
        .unwrap();
    drop(writer);
    let mut anchors = chain.anchors();
    let on_cut = anchors
        .iter_mut()
        .find(|anchor| anchor.seq == 10)
        .expect("the cut lies on the anchor at 10");
    on_cut.hash = "f".repeat(64);
    let verified = verify_with(&chain, &anchors);
    assert_eq!(
        verified.status,
        VerifyStatus::Broken {
            first_bad_seq: 11,
            reason: BreakReason::AnchorMismatch { anchor_seq: 10 },
        },
        "{verified:?}"
    );
}

#[test]
fn a_reported_end_before_the_record_of_the_run_still_finds_it() {
    // Das Ende, das der Schreiber vor dem Lauf gemeldet hat, liegt vor dem
    // `audit.pruned`; die Datei ist inzwischen ersetzt. Die Prüfung liest
    // weiter, bis sie den Beleg findet, statt einen Bruch zu melden.
    let (chain, writer, cutoff) = two_batches();
    let reported = writer.handle().sync().unwrap().seq;
    assert_eq!(reported, 13);
    writer
        .handle()
        .prune(cutoff, chain.anchors())
        .unwrap()
        .unwrap();
    let verified =
        AuditVerifier::verify_until(&chain.log, Some(&KEY), &chain.anchors(), Some(reported))
            .unwrap();
    drop(writer);
    assert!(verified.is_ok(), "{verified:?}");
    assert!(
        verified
            .warnings
            .contains(&VerifyWarning::Pruned { through_seq: 10 }),
        "{verified:?}"
    );
}

#[test]
fn a_pruned_record_forged_without_the_key_does_not_hold() {
    // Wer ohne Schlüssel vorn kürzt und einen passenden `audit.pruned`
    // dazuschreibt, scheitert am MAC.
    let chain = Chain::new();
    chain.write(5, 8);
    let lines = chain.lines();
    let first_kept = AuditRecord::from_line(lines[5].as_bytes()).unwrap();
    let last = AuditRecord::from_line(lines[9].as_bytes()).unwrap();
    let forged = humanitl_audit::RecordBody {
        seq: last.body.seq + 1,
        ts: last.body.ts.clone(),
        session: humanitl_audit::NO_SESSION.to_owned(),
        kind: "audit.pruned".to_owned(),
        data: humanitl_audit::AuditPruned {
            through_seq: 5,
            through_hash: first_kept.body.prev.clone(),
            records: 5,
            cutoff: Some(last.body.ts),
            max_bytes: None,
        }
        .data(),
        prev: last.hash,
    }
    .seal(&[1; 32])
    .unwrap();
    let mut kept = lines[5..].to_vec();
    kept.push(String::from_utf8(forged.to_line().unwrap()).unwrap());
    chain.set_lines(&kept);
    let verified = chain.verify();
    assert!(!verified.is_ok(), "{verified:?}");
    assert_eq!(
        verified.status,
        VerifyStatus::Broken {
            first_bad_seq: 6,
            reason: BreakReason::SeqGap,
        },
        "the forged record does not document the start: {verified:?}"
    );
}

#[test]
fn an_unanchored_record_of_the_run_deletes_nothing() {
    // Die Tabelle `audit_anchors` nimmt den Anker hinter `audit.pruned` nicht
    // an: Der Beleg läge dann im unverankerten Ende, also wird nicht
    // geschnitten, und der Lauf meldet `AUDIT_006`.
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use humanitl_audit::{AuditKey, KeyOrigin};
    use humanitl_audit_log::{AnchorMirror, WriterOptions};
    use humanitl_core::diagnostics::codes::RECORDER_001;
    use humanitl_core::{Diagnostic, Severity};

    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("audit.jsonl");
    let refuse = Arc::new(AtomicBool::new(false));
    let mirror: AnchorMirror = {
        let refuse = Arc::clone(&refuse);
        Box::new(move |_: &Anchor| {
            if refuse.load(Ordering::SeqCst) {
                Err(Diagnostic::builder(RECORDER_001, Severity::Error)
                    .why("the table is gone".to_owned())
                    .build())
            } else {
                Ok(())
            }
        })
    };
    let (writer, _) = AuditWriter::open(
        &log,
        &AuditKey::from_bytes(KEY, KeyOrigin::File),
        WriterOptions {
            anchor_every: 5,
            ..WriterOptions::default()
        },
        &[],
        Some(mirror),
    )
    .unwrap();
    write(&writer, 8);
    std::thread::sleep(Duration::from_millis(5));
    let cutoff = SystemTime::now();
    std::thread::sleep(Duration::from_millis(5));
    write(&writer, 3);

    refuse.store(true, Ordering::SeqCst);
    let refused = writer.handle().prune(cutoff, Vec::new()).unwrap_err();
    assert_eq!(refused.code.as_str(), "AUDIT_006", "{refused:?}");
    assert!(
        refused.why.contains("nothing was deleted"),
        "{}",
        refused.why
    );
    drop(writer);

    let text = std::fs::read_to_string(&log).unwrap();
    let first = AuditRecord::from_line(text.lines().next().unwrap().as_bytes()).unwrap();
    assert_eq!(first.body.seq, 1, "the start of the chain is still there");
    assert_eq!(
        text.lines().count(),
        15,
        "13 lines, the record of the run, its anchor"
    );
    let verified = AuditVerifier::verify(&log, Some(&KEY), &[]).unwrap();
    assert!(verified.is_ok(), "{verified:?}");
}

// HUM-194: die Grenze in Bytes, `audit.max_bytes`.

/// Nur die Grenze in Bytes, keine in der Zeit.
fn by_size(max_bytes: u64) -> humanitl_audit::PruneLimits {
    humanitl_audit::PruneLimits {
        before: None,
        max_bytes,
    }
}

/// Die Größe des Logs in Bytes.
fn size_of(chain: &Chain) -> u64 {
    std::fs::metadata(&chain.log).unwrap().len()
}

/// Die Länge der Zeile mit der Nummer `seq` samt Zeilenende, aus `lines`.
fn line_len(lines: &[String], seq: u64) -> u64 {
    let line = lines
        .iter()
        .find(|line| AuditRecord::from_line(line.as_bytes()).unwrap().body.seq == seq)
        .expect("the line is there");
    u64::try_from(line.len()).unwrap() + 1
}

#[test]
fn a_log_over_max_bytes_is_cut_below_it_and_verifies_with_pruned() {
    let chain = Chain::new();
    let writer = chain.open(5);
    write(&writer, 40);
    let before = chain.lines();
    let head = chain.records().last().unwrap().body.seq;
    let cap = size_of(&chain) / 2;

    let outcome = writer
        .handle()
        .prune_to(by_size(cap), chain.anchors())
        .unwrap();
    let report = outcome.cut.clone().expect("the log is twice the cap");
    writer.handle().sync().unwrap();

    let size = size_of(&chain);
    assert!(size <= cap, "{size} bytes stay over the cap of {cap}");
    assert_eq!(outcome.size, size, "the outcome names the size on disk");
    assert!(!outcome.over(cap));
    // Nicht mehr als nötig: Mit dem letzten gelöschten Record läge die Datei
    // über der Grenze.
    assert!(
        size + line_len(&before, report.through_seq) > cap,
        "the run deleted more than the cap asks: {report:?}, {size} of {cap}"
    );

    // Gelöscht ist ein Anfang; der Rest steht in seiner Reihenfolge, dahinter
    // der Record des Laufs und sein Anker.
    let records = chain.records();
    let seqs: Vec<u64> = records.iter().map(|record| record.body.seq).collect();
    let kept: Vec<u64> = (report.through_seq + 1..=head + 2).collect();
    assert_eq!(seqs, kept, "only the start went");
    let pruned = &records[records.len() - 2];
    assert_eq!(pruned.body.kind, "audit.pruned");
    assert_eq!(pruned.body.data["max_bytes"], cap);
    assert!(
        pruned.body.data.get("cutoff").is_none(),
        "no limit in time, no cutoff: {:?}",
        pruned.body.data
    );
    assert_eq!(pruned.body.data["through_seq"], report.through_seq);
    assert_eq!(records.last().unwrap().body.kind, "audit.anchor");

    let verified = chain.verify();
    assert_eq!(verified.status, VerifyStatus::Ok, "{verified:?}");
    assert_eq!(
        verified.warnings,
        [VerifyWarning::Pruned {
            through_seq: report.through_seq
        }],
        "{verified:?}"
    );

    // Der Schreiber setzt an der neuen Datei fort.
    write(&writer, 1);
    drop(writer);
    assert!(chain.verify().is_ok());
}

#[test]
fn a_broken_chain_is_not_cut_by_size_either() {
    let chain = Chain::new();
    let writer = chain.open(5);
    write(&writer, 20);
    let mut lines = chain.lines();
    lines[3] = lines[3].replacen("\"decision\":\"block\"", "\"decision\":\"allow\"", 1);
    chain.set_lines(&lines);
    let before = std::fs::read(&chain.log).unwrap();

    let refused = writer
        .handle()
        .prune_to(by_size(size_of(&chain) / 4), chain.anchors())
        .unwrap_err();
    assert_eq!(refused.code.as_str(), "AUDIT_001", "{refused:?}");
    assert!(refused.why.contains("deletes nothing"), "{}", refused.why);
    drop(writer);
    assert_eq!(
        std::fs::read(&chain.log).unwrap(),
        before,
        "the evidence of the break stays byte for byte"
    );
}

#[test]
fn max_bytes_zero_cuts_nothing() {
    let chain = Chain::new();
    let writer = chain.open(5);
    write(&writer, 20);
    let before = std::fs::read(&chain.log).unwrap();
    assert!(
        writer
            .handle()
            .prune_to(by_size(0), chain.anchors())
            .unwrap()
            .cut
            .is_none(),
        "0 means no cap"
    );
    drop(writer);
    assert_eq!(std::fs::read(&chain.log).unwrap(), before);
}

#[test]
fn a_log_at_max_bytes_stays_as_it_is() {
    let chain = Chain::new();
    let writer = chain.open(5);
    write(&writer, 20);
    let before = std::fs::read(&chain.log).unwrap();
    let cap = u64::try_from(before.len()).unwrap();
    assert!(
        writer
            .handle()
            .prune_to(by_size(cap), chain.anchors())
            .unwrap()
            .cut
            .is_none(),
        "a log exactly at the cap is not over it"
    );
    drop(writer);
    assert_eq!(std::fs::read(&chain.log).unwrap(), before);
}

#[test]
fn a_newest_record_larger_than_the_cap_stays_with_the_record_of_the_run() {
    // Festgelegt (HUM-194): Der neueste Record geht nach der Grenze in Bytes
    // nie. Ist er allein größer als sie, bleibt er mit `audit.pruned` und
    // dessen Anker stehen, die Datei liegt über der Grenze, und der Bericht
    // sagt es.
    let chain = Chain::new();
    let writer = chain.open(0);
    write(&writer, 6);
    let before = chain.lines();
    let head = chain.records().last().unwrap().body.seq;
    let cap = line_len(&before, head) / 2;

    let outcome = writer
        .handle()
        .prune_to(by_size(cap), chain.anchors())
        .unwrap();
    let report = outcome
        .cut
        .clone()
        .expect("everything before the newest record goes");
    writer.handle().sync().unwrap();
    assert_eq!(report.through_seq, head - 1, "all but the newest record");
    assert_eq!(report.records, head - 1);

    let records = chain.records();
    let kinds: Vec<&str> = records
        .iter()
        .map(|record| record.body.kind.as_str())
        .collect();
    assert_eq!(kinds, ["flow.decided", "audit.pruned", "audit.anchor"]);
    assert_eq!(records[0].body.seq, head, "the newest record stays");
    assert_eq!(outcome.size, size_of(&chain));
    assert!(outcome.over(cap), "the log stays over the cap, and says so");

    drop(writer);
    let verified = chain.verify();
    assert!(verified.is_ok(), "{verified:?}");
    assert_eq!(
        verified.warnings,
        [VerifyWarning::Pruned {
            through_seq: head - 1
        }]
    );
}

#[test]
fn age_and_size_together_take_the_further_cut() {
    // Nach Alter gingen die zehn Zeilen des ersten Schubs; die Grenze in
    // Bytes lässt nur die letzten beiden Records stehen. Es gilt der weitere
    // Schnitt, belegt von einem einzigen `audit.pruned` mit beiden Grenzen.
    let (chain, writer, cutoff) = two_batches();
    let lines = chain.lines();
    let cap = line_len(&lines, 12) + line_len(&lines, 13) + 1_000;

    let report = writer
        .handle()
        .prune_to(
            humanitl_audit::PruneLimits {
                before: Some(cutoff),
                max_bytes: cap,
            },
            chain.anchors(),
        )
        .unwrap()
        .cut
        .unwrap();
    writer.handle().sync().unwrap();
    assert_eq!(report.through_seq, 11, "{report:?}");
    assert!(size_of(&chain) <= cap);

    let records = chain.records();
    let pruned = &records[records.len() - 2];
    assert_eq!(pruned.body.kind, "audit.pruned");
    assert_eq!(pruned.body.data["max_bytes"], cap);
    assert!(
        pruned.body.data["cutoff"].is_string(),
        "{:?}",
        pruned.body.data
    );
    drop(writer);
    let verified = chain.verify();
    assert!(verified.is_ok(), "{verified:?}");
    assert_eq!(
        verified.warnings,
        [VerifyWarning::Pruned { through_seq: 11 }]
    );
}

#[test]
fn an_age_only_run_writes_the_record_it_wrote_before() {
    // Ohne Grenze in Bytes trägt `audit.pruned` kein `max_bytes`: derselbe
    // Record wie vor HUM-194.
    let (chain, writer, cutoff) = two_batches();
    writer
        .handle()
        .prune(cutoff, chain.anchors())
        .unwrap()
        .unwrap();
    writer.handle().sync().unwrap();
    drop(writer);
    let records = chain.records();
    let pruned = &records[records.len() - 2];
    let mut fields: Vec<&str> = pruned
        .body
        .data
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    fields.sort_unstable();
    assert_eq!(fields, ["cutoff", "records", "through_hash", "through_seq"]);
}

#[test]
fn a_log_that_cannot_be_cut_still_reports_its_size() {
    // Nur ein Record, und der ist größer als die Grenze: Es gibt nichts zu
    // schneiden, aber das Ergebnis nennt die Größe, und `over` meldet sie.
    let chain = Chain::new();
    let writer = chain.open(0);
    write(&writer, 1);
    let before = std::fs::read(&chain.log).unwrap();
    let cap = 1;

    let outcome = writer
        .handle()
        .prune_to(by_size(cap), chain.anchors())
        .unwrap();
    assert!(outcome.cut.is_none(), "the newest record never goes");
    assert_eq!(outcome.size, u64::try_from(before.len()).unwrap());
    assert!(outcome.over(cap), "over the cap without a cut");
    assert!(!outcome.over(0), "0 is no cap");
    drop(writer);
    assert_eq!(std::fs::read(&chain.log).unwrap(), before);
}

#[test]
fn the_newest_record_just_under_the_line_limit_fits_below_the_smallest_cap() {
    // Die kleinste Grenze, die die Konfiguration annimmt, mit einem neuesten
    // Record knapp unter `MAX_LINE_BYTES` und älteren Records davor: Danach
    // liegt das Log samt `audit.pruned` und Anker darunter.
    use humanitl_audit::kinds::DaemonStopped;
    use humanitl_audit::{MIN_MAX_BYTES, RecordKind};
    use humanitl_audit_log::writer::MAX_LINE_BYTES;

    let long = |len: u64| {
        RecordKind::DaemonStopped(DaemonStopped {
            reason: "x".repeat(usize::try_from(len).unwrap()),
        })
    };
    let chain = Chain::new();
    let writer = chain.open(0);
    for _ in 0..4 {
        writer.handle().record(None, long(600_000));
    }
    writer.handle().record(None, long(MAX_LINE_BYTES - 400));
    writer.handle().sync().unwrap();
    let lines = chain.lines();
    let head = u64::try_from(lines.len()).unwrap();
    let newest = line_len(&lines, head);
    assert!(
        newest > MAX_LINE_BYTES - 400 && newest <= MAX_LINE_BYTES,
        "the newest line is just under the limit: {newest}"
    );
    assert!(size_of(&chain) > MIN_MAX_BYTES, "the log is over the cap");

    let outcome = writer
        .handle()
        .prune_to(by_size(MIN_MAX_BYTES), chain.anchors())
        .unwrap();
    writer.handle().sync().unwrap();
    let report = outcome.cut.clone().expect("the older records go");
    let size = size_of(&chain);
    assert!(
        size <= MIN_MAX_BYTES,
        "{size} bytes stay over the cap of {MIN_MAX_BYTES}"
    );
    assert!(!outcome.over(MIN_MAX_BYTES), "{outcome:?}");
    assert_eq!(chain.records()[0].body.seq, report.through_seq + 1);
    drop(writer);
    let verified = chain.verify();
    assert!(verified.is_ok(), "{verified:?}");
}
