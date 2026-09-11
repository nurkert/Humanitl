//! Was die Tests der Audit-Kette gemeinsam brauchen: ein Log in einem
//! Wegwerf-Verzeichnis, dessen Anker in einer echten Tabelle `audit_anchors`
//! landen, genau wie im Daemon.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use humanitl_audit::kinds::FlowDecided;
use humanitl_audit::{
    Anchor, AnchorMirror, AuditKey, AuditRecord, AuditVerifier, AuditWriter, KeyOrigin, RecordKind,
    VerifyReport, WriterOptions,
};
use humanitl_core::{Decision, DecisionSource, Diagnostic, FlowId};
use humanitl_recorder::{AnchorStore, AuditAnchor, read_anchors};

/// Der Schlüssel der Tests.
pub const KEY: [u8; 32] = [7; 32];

/// Ein Log, seine Datenbank und sein Schlüssel.
pub struct Chain {
    pub dir: tempfile::TempDir,
    pub log: PathBuf,
    pub db: PathBuf,
    pub key: AuditKey,
}

impl Chain {
    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("audit").join("audit.jsonl");
        let db = dir.path().join("humanitl.db");
        Self {
            dir,
            log,
            db,
            key: AuditKey::from_bytes(KEY, KeyOrigin::File),
        }
    }

    /// Öffnet einen Schreiber, der seine Anker in die Tabelle legt.
    pub fn try_open(
        &self,
        anchor_every: u32,
    ) -> Result<(AuditWriter, Vec<Diagnostic>), Diagnostic> {
        self.try_open_with(&self.key, anchor_every)
    }

    pub fn try_open_with(
        &self,
        key: &AuditKey,
        anchor_every: u32,
    ) -> Result<(AuditWriter, Vec<Diagnostic>), Diagnostic> {
        let store = AnchorStore::open(&self.db).unwrap();
        let mirror: AnchorMirror = Box::new(move |anchor: &Anchor| {
            store.put(&AuditAnchor {
                seq: anchor.seq,
                hash: anchor.hash.clone(),
                ts: anchor.ts.clone(),
            })
        });
        AuditWriter::open(
            &self.log,
            key,
            WriterOptions {
                anchor_every,
                ..WriterOptions::default()
            },
            &self.anchors(),
            Some(mirror),
        )
    }

    pub fn open(&self, anchor_every: u32) -> AuditWriter {
        let (writer, notes) = self.try_open(anchor_every).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        writer
    }

    /// Schreibt `users` Records über einen frischen Schreiber und schließt ihn
    /// ohne `daemon.stopped`.
    pub fn write(&self, anchor_every: u32, users: usize) {
        let writer = self.open(anchor_every);
        let handle = writer.handle();
        for index in 0..users {
            handle.record(None, decided(index));
        }
        handle.sync().unwrap();
        drop(writer);
    }

    /// Die Anker aus der Tabelle.
    pub fn anchors(&self) -> Vec<Anchor> {
        read_anchors(&self.db)
            .unwrap()
            .into_iter()
            .map(|anchor| Anchor {
                seq: anchor.seq,
                hash: anchor.hash,
                ts: anchor.ts,
            })
            .collect()
    }

    pub fn lines(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    pub fn set_lines(&self, lines: &[String]) {
        let mut text = lines.join("\n");
        if !lines.is_empty() {
            text.push('\n');
        }
        std::fs::write(&self.log, text).unwrap();
    }

    pub fn records(&self) -> Vec<AuditRecord> {
        self.lines()
            .iter()
            .map(|line| AuditRecord::from_line(line.as_bytes()).unwrap())
            .collect()
    }

    pub fn verify(&self) -> VerifyReport {
        AuditVerifier::verify(&self.log, Some(&KEY), &self.anchors()).unwrap()
    }
}

/// Ein `flow.decided`, abwechselnd `allow` und `block`.
pub fn decided(index: usize) -> RecordKind {
    let decision = if index.is_multiple_of(2) {
        Decision::Allow
    } else {
        Decision::Block {
            reason: humanitl_core::BlockReason::User,
            note: None,
        }
    };
    RecordKind::FlowDecided(FlowDecided::new(
        FlowId::new(),
        &decision,
        DecisionSource::User,
    ))
}
