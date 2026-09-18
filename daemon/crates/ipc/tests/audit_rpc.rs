//! Die `Audit`-RPC über einer echten Kette (HUM-156).
//!
//! Jeder Test schreibt eine Kette mit dem Schreiber des Daemons in ein
//! Wegwerf-Verzeichnis, samt Ankern in einer echten Tabelle `audit_anchors`,
//! und fragt den Dienst so, wie Oberfläche und Kommandozeile ihn fragen.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use humanitl_audit::kinds::{ConfigChanged, PseudonymCreated};
use humanitl_audit::{
    Anchor, AnchorMirror, AuditKey, AuditRecord, AuditWriter, KeyOrigin, RecordKind, WriterOptions,
};
use humanitl_config::Config;
use humanitl_core::SessionId;
use humanitl_ipc::v1::audit_request::{Export, Op, Query};
use humanitl_ipc::v1::humanitl_server::Humanitl as _;
use humanitl_ipc::{AuditService, IpcServer, diagnostic_from_status, v1};
use humanitl_recorder::{AnchorStore, AuditAnchor, read_anchors};
use tonic::{Code, Request};

/// Der Schlüssel des Daemons in diesen Tests.
const KEY: [u8; 32] = [9; 32];

/// Eine Kette in einem Wegwerf-Verzeichnis.
struct Chain {
    dir: tempfile::TempDir,
    log: PathBuf,
    db: PathBuf,
}

impl Chain {
    /// Schreibt `records` Records, abwechselnd `config.changed` und
    /// `pseudonym.created`, die geraden in `session`; Anker alle drei.
    ///
    /// Der Schreiber endet danach mit `daemon.stopped` und einem Anker, wie
    /// ein Daemon, der geordnet geht.
    fn write(records: usize, session: SessionId) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("audit").join("audit.jsonl");
        let db = dir.path().join("humanitl.db");
        let store = AnchorStore::open(&db).unwrap();
        let mirror: AnchorMirror = Box::new(move |anchor: &Anchor| {
            store.put(&AuditAnchor {
                seq: anchor.seq,
                hash: anchor.hash.clone(),
                ts: anchor.ts.clone(),
            })
        });
        let (writer, notes) = AuditWriter::open(
            &log,
            &key(),
            WriterOptions {
                anchor_every: 3,
                fsync_every: 1,
                ..WriterOptions::default()
            },
            &[],
            Some(mirror),
        )
        .unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        let handle = writer.handle();
        for index in 0..records {
            let kind = if index % 2 == 0 {
                RecordKind::ConfigChanged(ConfigChanged {
                    key: format!("hold.timeout_secs.{index}"),
                    origin: "cli".to_owned(),
                    secret: false,
                    value: Some("a, \"quoted\" value".to_owned()),
                })
            } else {
                RecordKind::PseudonymCreated(PseudonymCreated {
                    pseudonym: format!("EMAIL_{index}"),
                    kind: "email".to_owned(),
                })
            };
            handle.record((index % 2 == 0).then_some(session), kind);
        }
        writer.stop("test").expect("the writer stops");
        Self { dir, log, db }
    }

    /// Der Dienst über dieser Kette, mit dem Schlüssel des Schreibers.
    fn server(&self) -> IpcServer {
        self.server_with(key())
    }

    fn server_with(&self, key: AuditKey) -> IpcServer {
        IpcServer::over_the_recording(&Config::default(), None).with_audit_log(AuditService::new(
            self.log.clone(),
            self.db.clone(),
            Arc::new(key),
        ))
    }

    fn lines(&self) -> Vec<String> {
        std::fs::read_to_string(&self.log)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn records(&self) -> Vec<AuditRecord> {
        self.lines()
            .iter()
            .map(|line| AuditRecord::from_line(line.as_bytes()).unwrap())
            .collect()
    }

    fn set_lines(&self, lines: &[String]) {
        std::fs::write(&self.log, format!("{}\n", lines.join("\n"))).unwrap();
    }

    fn anchors(&self) -> Vec<AuditAnchor> {
        read_anchors(&self.db).unwrap()
    }

    fn out(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }
}

fn key() -> AuditKey {
    AuditKey::from_bytes(KEY, KeyOrigin::File)
}

async fn ask(server: &IpcServer, op: Op) -> Result<v1::AuditResponse, tonic::Status> {
    server
        .audit(Request::new(v1::AuditRequest { op: Some(op) }))
        .await
        .map(tonic::Response::into_inner)
}

async fn answer(server: &IpcServer, op: Op) -> v1::AuditResponse {
    ask(server, op).await.expect("the daemon answers")
}

fn export(format: &str, out: &Path) -> Op {
    Op::Export(Export {
        format: format.to_owned(),
        out_path: out.display().to_string(),
        ..Export::default()
    })
}

/// `verify` prüft mit Schlüssel und Ankern und nennt denselben Kopf wie
/// `head`: den Hash der letzten Zeile.
#[tokio::test]
async fn verify_and_head_name_the_same_head_and_the_anchors() {
    let chain = Chain::write(7, SessionId::new());
    let server = chain.server();
    let last = chain.records().pop().unwrap();
    let anchors = chain.anchors();
    assert!(anchors.len() >= 3, "{anchors:?}");

    let verified = answer(&server, Op::Verify(())).await;
    assert!(verified.ok, "{verified:?}");
    assert_eq!(verified.entries, last.body.seq);
    assert_eq!(verified.head_seq, last.body.seq);
    assert_eq!(hex::encode(&verified.head_hash), last.hash);
    assert!(
        verified.warnings.is_empty(),
        "with key and a final anchor nothing stays unproven: {:?}",
        verified.warnings
    );
    assert!(verified.anchors_reported);
    assert_eq!(verified.anchors, u64::try_from(anchors.len()).unwrap());
    assert!(verified.diagnostic.is_none());

    let head = answer(&server, Op::Head(())).await;
    assert!(head.ok);
    assert_eq!(head.head_hash, verified.head_hash);
    assert_eq!(head.head_seq, verified.head_seq);
    assert_eq!(head.entries, verified.entries);
    assert_eq!(head.anchors, verified.anchors);
    assert!(head.anchors_reported);
    let last_anchor = anchors.last().unwrap();
    let at = head.last_anchor_at.expect("the last anchor has a time");
    let expected = chrono::DateTime::parse_from_rfc3339(&last_anchor.ts).unwrap();
    assert_eq!(at.seconds, expected.timestamp());
    assert_eq!(
        u32::try_from(at.nanos).unwrap(),
        expected.timestamp_subsec_nanos()
    );
}

/// Eine nach dem Schreiben veränderte Zeile ist ein Bruch ab ihrer Nummer,
/// mit Grund und Befund; die Antwort ist trotzdem eine Antwort.
#[tokio::test]
async fn a_changed_line_breaks_at_its_seq_with_the_reason() {
    let chain = Chain::write(7, SessionId::new());
    let mut lines = chain.lines();
    // Zeile 5: `pseudonym.created` mit `EMAIL_3` (Anker bei 3 und 6).
    let changed = lines[4].replacen("EMAIL_3", "EMAIL_X", 1);
    assert_ne!(changed, lines[4], "the fixture must really change");
    lines[4] = changed;
    chain.set_lines(&lines);

    let verified = answer(&chain.server(), Op::Verify(())).await;
    assert!(!verified.ok);
    assert_eq!(verified.first_bad_seq, 5);
    assert_eq!(verified.break_reason, "hash_mismatch");
    assert_eq!(verified.entries, 4, "four records before it hold");
    assert_eq!(
        verified.head_seq, 4,
        "the head is the last record that held"
    );
    let diagnostic = verified.diagnostic.expect("the finding travels");
    assert_eq!(diagnostic.code, "AUDIT_001");
    assert!(
        diagnostic.why.contains("hash_mismatch at seq 5"),
        "{}",
        diagnostic.why
    );
}

/// Die Prüfung rechnet mit dem Schlüssel des Daemons: Eine Kette, deren MACs
/// ein anderer Schlüssel gerechnet hat, bricht beim ersten Record. Ohne
/// Schlüssel fiele das nicht auf.
#[tokio::test]
async fn verify_checks_the_mac_with_the_key_of_the_daemon() {
    let chain = Chain::write(4, SessionId::new());
    let verified = answer(
        &chain.server_with(AuditKey::from_bytes([1; 32], KeyOrigin::File)),
        Op::Verify(()),
    )
    .await;
    assert!(!verified.ok);
    assert_eq!(verified.first_bad_seq, 1);
    assert_eq!(verified.break_reason, "mac_mismatch");
}

/// Die Prüfung liest die Anker: Ein Log, das unter seinen letzten Anker
/// gekürzt wurde, ist gebrochen. Die Datei allein sähe heil aus.
#[tokio::test]
async fn verify_reads_the_anchors_and_sees_a_cut_below_one() {
    let chain = Chain::write(7, SessionId::new());
    let mut lines = chain.lines();
    lines.pop();
    chain.set_lines(&lines);

    let verified = answer(&chain.server(), Op::Verify(())).await;
    assert!(!verified.ok);
    assert_eq!(verified.break_reason, "truncated_below_anchor");
}

/// Seiten kommen neueste zuerst; der Cursor ist die Nummer des untersten
/// Records, und die letzte Seite hat keinen.
#[tokio::test]
async fn query_pages_newest_first_with_a_cursor() {
    let chain = Chain::write(9, SessionId::new());
    let server = chain.server();
    let all = chain.records();
    let total = u64::try_from(all.len()).unwrap();

    let mut seen = Vec::new();
    let mut cursor = String::new();
    let mut pages = 0;
    loop {
        let page = answer(
            &server,
            Op::Query(Query {
                limit: 4,
                cursor: cursor.clone(),
                ..Query::default()
            }),
        )
        .await;
        assert!(page.ok);
        assert_eq!(page.entries, total, "the total counts every page");
        assert!(page.records.len() <= 4);
        seen.extend(page.records.iter().map(|entry| entry.seq));
        pages += 1;
        if page.next_cursor.is_empty() {
            break;
        }
        assert_eq!(
            page.next_cursor,
            page.records.last().unwrap().seq.to_string()
        );
        cursor = page.next_cursor;
    }
    let expected: Vec<u64> = all.iter().rev().map(|record| record.body.seq).collect();
    assert_eq!(seen, expected);
    assert_eq!(pages, all.len().div_ceil(4));

    // Die Zeile reist Byte für Byte, `data` als JSON daneben.
    let first = answer(
        &server,
        Op::Query(Query {
            limit: 1,
            ..Query::default()
        }),
    )
    .await;
    let entry = &first.records[0];
    assert_eq!(entry.line, *chain.lines().last().unwrap());
    let data: serde_json::Value = serde_json::from_str(&entry.data_json).unwrap();
    assert_eq!(data, all.last().unwrap().body.data);
}

/// Art, Sitzung und Zeitraum schränken ein, und `entries` zählt die Treffer.
#[tokio::test]
async fn query_filters_by_kind_session_and_time() {
    let session = SessionId::new();
    let chain = Chain::write(6, session);
    let server = chain.server();
    let all = chain.records();

    let kind = answer(
        &server,
        Op::Query(Query {
            kind_prefix: "pseudonym.".to_owned(),
            ..Query::default()
        }),
    )
    .await;
    assert_eq!(kind.entries, 3);
    assert!(
        kind.records
            .iter()
            .all(|entry| entry.kind == "pseudonym.created")
    );

    let by_session = answer(
        &server,
        Op::Query(Query {
            session: session.to_string(),
            ..Query::default()
        }),
    )
    .await;
    assert_eq!(by_session.entries, 3);
    assert!(
        by_session
            .records
            .iter()
            .all(|entry| entry.session == session.to_string())
    );

    // Genau ein Zeitpunkt, beide Grenzen einschließlich: der Record selbst.
    let pick = &all[2];
    let at = chrono::DateTime::parse_from_rfc3339(&pick.body.ts).unwrap();
    let stamp = prost_types::Timestamp {
        seconds: at.timestamp(),
        nanos: i32::try_from(at.timestamp_subsec_nanos()).unwrap(),
    };
    let window = answer(
        &server,
        Op::Query(Query {
            from: Some(stamp),
            to: Some(stamp),
            ..Query::default()
        }),
    )
    .await;
    let same_time = all
        .iter()
        .filter(|record| record.body.ts == pick.body.ts)
        .count();
    assert_eq!(window.entries, u64::try_from(same_time).unwrap());
    assert!(
        window
            .records
            .iter()
            .any(|entry| entry.seq == pick.body.seq)
    );
}

/// JSONL ist die Kette Byte für Byte, CSV hat die zwölf Spalten aus HUM-050,
/// und ein vorhandener Export wird nicht überschrieben.
#[tokio::test]
async fn export_writes_the_chain_and_the_twelve_columns_and_overwrites_nothing() {
    let chain = Chain::write(5, SessionId::new());
    let server = chain.server();

    let jsonl = chain.out("export.jsonl");
    let written = answer(&server, export("jsonl", &jsonl)).await;
    assert!(written.ok);
    assert_eq!(written.out_path, jsonl.display().to_string());
    assert_eq!(written.entries, u64::try_from(chain.lines().len()).unwrap());
    assert_eq!(
        std::fs::read(&jsonl).unwrap(),
        std::fs::read(&chain.log).unwrap()
    );

    let csv = chain.out("export.csv");
    let written = answer(&server, export("csv", &csv)).await;
    assert!(written.ok);
    let text = std::fs::read_to_string(&csv).unwrap();
    let mut rows = text.split("\r\n");
    assert_eq!(
        rows.next(),
        Some("seq,ts,session,kind,flow,host,method,decision,rule,status,size,hash")
    );
    let first = chain.records().remove(0);
    let row = rows.next().unwrap();
    assert!(row.starts_with(&format!("1,{},", first.body.ts)), "{row}");
    assert!(row.ends_with(&first.hash), "{row}");

    let refused = ask(&server, export("jsonl", &jsonl)).await.unwrap_err();
    let diagnostic = diagnostic_from_status(&refused).unwrap();
    assert_eq!(diagnostic.code, "AUDIT_008");
    assert_eq!(
        std::fs::read(&jsonl).unwrap(),
        std::fs::read(&chain.log).unwrap(),
        "the first export stays as it was"
    );
}

/// Der Zeitraum des Exports schließt beide Grenzen ein.
#[tokio::test]
async fn export_takes_the_range_inclusively() {
    let chain = Chain::write(5, SessionId::new());
    let all = chain.records();
    let stamp = |record: &AuditRecord| {
        let at = chrono::DateTime::parse_from_rfc3339(&record.body.ts).unwrap();
        prost_types::Timestamp {
            seconds: at.timestamp(),
            nanos: i32::try_from(at.timestamp_subsec_nanos()).unwrap(),
        }
    };
    let out = chain.out("range.jsonl");
    let written = answer(
        &chain.server(),
        Op::Export(Export {
            format: "jsonl".to_owned(),
            out_path: out.display().to_string(),
            from: Some(stamp(&all[1])),
            to: Some(stamp(&all[3])),
            ..Export::default()
        }),
    )
    .await;
    let expected: Vec<String> = chain
        .lines()
        .into_iter()
        .filter(|line| {
            let record = AuditRecord::from_line(line.as_bytes()).unwrap();
            record.body.ts >= all[1].body.ts && record.body.ts <= all[3].body.ts
        })
        .collect();
    assert!(expected.len() >= 3, "{expected:?}");
    assert_eq!(written.entries, u64::try_from(expected.len()).unwrap());
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        format!("{}\n", expected.join("\n"))
    );
}

/// Was an der Anfrage nicht stimmt, ist `AUDIT_009` mit `InvalidArgument`,
/// und es entsteht keine Datei.
#[tokio::test]
async fn a_request_that_does_not_hold_is_audit_009_and_writes_nothing() {
    let chain = Chain::write(2, SessionId::new());
    let server = chain.server();
    let relative = Op::Export(Export {
        format: "csv".to_owned(),
        out_path: "export.csv".to_owned(),
        ..Export::default()
    });
    for op in [
        relative,
        export("xml", &chain.out("export.xml")),
        Op::Query(Query {
            cursor: "next".to_owned(),
            ..Query::default()
        }),
    ] {
        let refused = ask(&server, op).await.unwrap_err();
        assert_eq!(refused.code(), Code::InvalidArgument, "{refused:?}");
        assert_eq!(diagnostic_from_status(&refused).unwrap().code, "AUDIT_009");
    }
    assert!(!chain.out("export.xml").exists());
    let none = server
        .audit(Request::new(v1::AuditRequest { op: None }))
        .await
        .unwrap_err();
    assert_eq!(diagnostic_from_status(&none).unwrap().code, "AUDIT_009");
}

/// Ein Daemon ohne Audit-Log sagt das mit `IPC_006`, wie der Fake, statt eine
/// leere, heile Kette zu behaupten.
#[tokio::test]
async fn a_daemon_without_an_audit_log_says_so() {
    let server = IpcServer::over_the_recording(&Config::default(), None);
    for op in [Op::Verify(()), Op::Head(()), Op::Query(Query::default())] {
        let refused = ask(&server, op).await.unwrap_err();
        assert_eq!(diagnostic_from_status(&refused).unwrap().code, "IPC_006");
    }
    let info = server
        .get_info(Request::new(()))
        .await
        .unwrap()
        .into_inner();
    assert!(!info.capabilities.contains(&"audit".to_owned()));
    let with = Chain::write(1, SessionId::new()).server();
    let info = with.get_info(Request::new(())).await.unwrap().into_inner();
    assert!(info.capabilities.contains(&"audit".to_owned()));
}

/// Unter `PrivateTmp` lehnt der Dienst einen Export nach `/tmp` ab, bevor er
/// irgendetwas schreibt.
#[tokio::test]
async fn an_export_into_a_private_tmp_is_refused_by_the_service() {
    let chain = Chain::write(2, SessionId::new());
    let mounts = "412 33 0:30 /tmp/systemd-private-4f1e-humanitld.service-Q2c3/tmp /tmp rw \
                  shared:210 - ext4 /dev/sda1 rw\n";
    let server = IpcServer::over_the_recording(&Config::default(), None).with_audit_log(
        AuditService::new(chain.log.clone(), chain.db.clone(), Arc::new(key()))
            .with_mountinfo(mounts.to_owned()),
    );
    let refused = ask(
        &server,
        export("jsonl", Path::new("/tmp/hum-156-private.jsonl")),
    )
    .await
    .unwrap_err();
    let diagnostic = diagnostic_from_status(&refused).unwrap();
    assert_eq!(diagnostic.code, "AUDIT_008");
    assert!(diagnostic.why.contains("PrivateTmp"), "{}", diagnostic.why);
}

/// Eine Zeile, die hinter dem gemeldeten Ende gerade entsteht, ist kein Bruch:
/// Prüfung, Kopf, Seite und Export lesen bis zu dem Record, den der laufende
/// Schreiber zuletzt als geschrieben gemeldet hat, und nicht weiter.
#[tokio::test]
async fn a_line_being_written_behind_the_head_is_no_break() {
    let dir = tempfile::tempdir().unwrap();
    let log = dir.path().join("audit").join("audit.jsonl");
    let db = dir.path().join("humanitl.db");
    let store = AnchorStore::open(&db).unwrap();
    let mirror: AnchorMirror = Box::new(move |anchor: &Anchor| {
        store.put(&AuditAnchor {
            seq: anchor.seq,
            hash: anchor.hash.clone(),
            ts: anchor.ts.clone(),
        })
    });
    let (writer, _) = AuditWriter::open(
        &log,
        &key(),
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
            RecordKind::PseudonymCreated(PseudonymCreated {
                pseudonym: format!("EMAIL_{index}"),
                kind: "email".to_owned(),
            }),
        );
    }
    let head = writer.handle().sync().expect("the writer runs");
    let written = std::fs::read(&log).unwrap();

    // Hinter dem gemeldeten Ende: eine ganze Zeile, die der Schreiber nach
    // `sync` schon geschrieben hat (hier eine Kopie der letzten), und der
    // Anfang einer Zeile ohne ihr Ende, wie ihn ein Leser mitten in einem
    // Schreibvorgang sieht.
    {
        use std::io::Write as _;
        let last_line = written
            .strip_suffix(b"\n")
            .and_then(|body| body.rsplit(|byte| *byte == b'\n').next())
            .unwrap()
            .to_vec();
        let mut file = std::fs::OpenOptions::new().append(true).open(&log).unwrap();
        file.write_all(&last_line).unwrap();
        file.write_all(b"\n{\"data\":{\"pseudonym\":\"EMA").unwrap();
    }
    // Und ein Anker, den der Schreiber nach dem gemeldeten Ende schon in die
    // Tabelle gelegt hat: Er gehört zu einem Record, den diese Operation noch
    // nicht sieht, und ist kein Zeichen für ein gekürztes Log.
    AnchorStore::open(&db)
        .unwrap()
        .put(&AuditAnchor {
            seq: head.seq + 1,
            hash: "0".repeat(64),
            ts: "2026-09-18T10:00:00.000000Z".to_owned(),
        })
        .unwrap();
    let server = IpcServer::over_the_recording(&Config::default(), None).with_audit_log(
        AuditService::new(log.clone(), db.clone(), Arc::new(key())).with_writer(writer.handle()),
    );

    let verified = answer(&server, Op::Verify(())).await;
    assert!(verified.ok, "{verified:?}");
    assert_eq!(verified.head_seq, head.seq);
    assert_eq!(hex::encode(&verified.head_hash), head.hash);
    assert!(verified.diagnostic.is_none());

    let top = answer(&server, Op::Head(())).await;
    assert_eq!(top.head_seq, head.seq);
    assert_eq!(top.entries, head.seq);
    let anchored = read_anchors(&db)
        .unwrap()
        .iter()
        .filter(|anchor| anchor.seq <= head.seq)
        .count();
    assert_eq!(
        top.anchors,
        u64::try_from(anchored).unwrap(),
        "an anchor behind the head is not counted yet"
    );

    let page = answer(&server, Op::Query(Query::default())).await;
    assert_eq!(page.entries, head.seq);

    let out = dir.path().join("live.jsonl");
    let exported = answer(&server, export("jsonl", &out)).await;
    assert_eq!(exported.entries, head.seq);
    assert_eq!(
        std::fs::read(&out).unwrap(),
        written,
        "the export stops at the head"
    );
    drop(writer);
}
