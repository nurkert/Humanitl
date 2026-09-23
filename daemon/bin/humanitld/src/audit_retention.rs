//! Die Aufbewahrung der Audit-Kette im Daemon (HUM-157). Nur Verdrahtung.
//!
//! Was gelöscht wird und welcher Record es dokumentiert, entscheidet
//! `humanitl_audit::retention`; der Lauf selbst geschieht im Schreib-Thread
//! des Audit-Logs. Hier steht nur, was der Daemon beides kennt: die Zahl aus
//! `audit.retention_days` und die Anker aus `audit_anchors` der Aufzeichnung,
//! gegen die die Kette vor dem Löschen geprüft wird. Die Audit-Crate darf die
//! Aufzeichnung nicht kennen (`backlog/CONVENTIONS.md` 3.1).
//!
//! `0` heißt für immer, wie bei `recorder.retention_days`: Es gibt dann keine
//! Grenze, keinen Lauf und keinen Record. Der Lauf teilt sich den Takt mit
//! dem Aufräumen der Aufzeichnung: einmal beim Start, danach täglich.

use std::path::PathBuf;
use std::time::SystemTime;

use humanitl_audit::{Anchor, AuditHandle, PruneReport};
use humanitl_core::Diagnostic;
use humanitl_recorder::{Retention, read_anchors};

/// Was ein Lauf braucht: die Frist und die Datenbank mit den Ankern.
#[derive(Debug, Clone)]
pub(crate) struct AuditRetention {
    days: u32,
    db: PathBuf,
}

impl AuditRetention {
    /// Die Frist aus `audit.retention_days` und die Datenbank der
    /// Aufzeichnung.
    pub(crate) const fn new(days: u32, db: PathBuf) -> Self {
        Self { days, db }
    }

    /// Ein Lauf: löscht jeden Record, der vor `now` minus der Frist
    /// geschrieben wurde, und hinterlässt `audit.pruned`. Blockiert.
    ///
    /// `Ok(None)` bei der Frist `0` oder wenn kein Record alt genug war.
    ///
    /// # Errors
    ///
    /// `RECORDER_00x`, wenn sich die Anker nicht lesen lassen, sonst was
    /// [`AuditHandle::prune`] meldet: `AUDIT_001` für eine Kette, die nicht
    /// hält, `AUDIT_006` für Lesen und Schreiben. In keinem dieser Fälle ist
    /// etwas gelöscht.
    pub(crate) fn run_once(
        &self,
        audit: &AuditHandle,
        now: SystemTime,
    ) -> Result<Option<PruneReport>, Diagnostic> {
        let Some(horizon) = Retention::from_days(self.days).horizon(now) else {
            return Ok(None);
        };
        let anchors = read_anchors(&self.db)?
            .into_iter()
            .map(|anchor| Anchor {
                seq: anchor.seq,
                hash: anchor.hash,
                ts: anchor.ts,
            })
            .collect();
        audit.prune(horizon, anchors)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::time::{Duration, SystemTime};

    use humanitl_audit::kinds::DaemonStopped;
    use humanitl_audit::{
        Anchor, AnchorMirror, AuditKey, AuditRecord, AuditVerifier, AuditWriter, KeyOrigin,
        RecordKind, VerifyWarning, WriterOptions,
    };
    use humanitl_recorder::{AnchorStore, AuditAnchor, Recorder, RecorderSettings, read_anchors};

    use super::AuditRetention;

    const DAY: Duration = Duration::from_secs(24 * 60 * 60);
    const KEY: [u8; 32] = [9; 32];

    /// Ein Audit-Log mit Ankern in einer echten Tabelle `audit_anchors`, wie
    /// `open_audit` es verdrahtet, und vier Records samt zwei Ankern darin.
    fn setup() -> (tempfile::TempDir, AuditWriter, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("humanitl.db");
        // Das Öffnen der Aufzeichnung legt das Schema samt `audit_anchors` an.
        drop(
            Recorder::open(
                &db,
                &dir.path().join("blobs"),
                RecorderSettings::new(64, 4_096, 0),
            )
            .unwrap(),
        );
        let store = AnchorStore::open(&db).unwrap();
        let mirror: AnchorMirror = Box::new(move |anchor: &Anchor| {
            store.put(&AuditAnchor {
                seq: anchor.seq,
                hash: anchor.hash.clone(),
                ts: anchor.ts.clone(),
            })
        });
        let (writer, _) = AuditWriter::open(
            &dir.path().join("audit").join("audit.jsonl"),
            &AuditKey::from_bytes(KEY, KeyOrigin::File),
            WriterOptions {
                anchor_every: 3,
                ..WriterOptions::default()
            },
            &[],
            Some(mirror),
        )
        .unwrap();
        for index in 0..4 {
            writer.handle().record(
                None,
                RecordKind::DaemonStopped(DaemonStopped {
                    reason: format!("record {index}"),
                }),
            );
        }
        writer.handle().sync().unwrap();
        (dir, writer, db)
    }

    fn anchors(db: &std::path::Path) -> Vec<Anchor> {
        read_anchors(db)
            .unwrap()
            .into_iter()
            .map(|anchor| Anchor {
                seq: anchor.seq,
                hash: anchor.hash,
                ts: anchor.ts,
            })
            .collect()
    }

    #[test]
    fn a_run_past_the_horizon_prunes_and_the_chain_still_holds() {
        let (_dir, writer, db) = setup();
        let path = writer.path().to_path_buf();
        let anchors_before = anchors(&db);
        assert!(!anchors_before.is_empty(), "anchor_every = 3 anchored");

        // Ein Jahr und ein Tag später, bei einem Jahr Frist: Alles ist alt.
        let later = SystemTime::now() + 366 * DAY;
        let report = AuditRetention::new(365, db.clone())
            .run_once(&writer.handle(), later)
            .unwrap()
            .expect("every record is older than the horizon");
        // Vier Records und ihre Anker bei 3 und 6.
        assert_eq!(report.records, 6, "four records and two anchors went");
        assert_eq!(report.through_seq, 6);

        writer.handle().sync().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let records: Vec<AuditRecord> = text
            .lines()
            .map(|line| AuditRecord::from_line(line.as_bytes()).unwrap())
            .collect();
        assert_eq!(records[0].body.kind, "audit.pruned", "{text}");
        assert_eq!(records[0].body.seq, 7);
        assert_eq!(
            records[1].body.kind, "audit.anchor",
            "the record is anchored"
        );

        let anchors_after = anchors(&db);
        assert!(
            anchors_before
                .iter()
                .all(|anchor| anchors_after.contains(anchor)),
            "no anchor was deleted: {anchors_before:?} {anchors_after:?}"
        );
        let verified = AuditVerifier::verify(&path, Some(&KEY), &anchors_after).unwrap();
        assert!(verified.is_ok(), "{verified:?}");
        assert!(
            verified
                .warnings
                .contains(&VerifyWarning::Pruned { through_seq: 6 }),
            "{verified:?}"
        );
        drop(writer);
    }

    #[test]
    fn zero_days_prunes_nothing_and_writes_nothing() {
        let (_dir, writer, db) = setup();
        let path = writer.path().to_path_buf();
        let before = std::fs::read(&path).unwrap();
        let later = SystemTime::now() + 3_650 * DAY;
        assert!(
            AuditRetention::new(0, db)
                .run_once(&writer.handle(), later)
                .unwrap()
                .is_none(),
            "0 means forever"
        );
        writer.handle().sync().unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the log is untouched"
        );
        drop(writer);
    }

    #[test]
    fn records_younger_than_the_horizon_stay() {
        let (_dir, writer, db) = setup();
        let path = writer.path().to_path_buf();
        let before = std::fs::read(&path).unwrap();
        assert!(
            AuditRetention::new(1, db)
                .run_once(&writer.handle(), SystemTime::now())
                .unwrap()
                .is_none(),
            "nothing is a day old"
        );
        writer.handle().sync().unwrap();
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "the log is untouched"
        );
        drop(writer);
    }
}
