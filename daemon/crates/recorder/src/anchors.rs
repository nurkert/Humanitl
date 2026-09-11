//! Die zweite Ablage der Audit-Anker: die Tabelle `audit_anchors` (HUM-050).
//!
//! Der Schreiber der Audit-Kette legt jeden Anker zweimal ab: als Record in
//! `audit.jsonl` und hier. Er schreibt über eine eigene Verbindung und nicht
//! über den Schreib-Thread der Aufzeichnung, weil ein Anker erst dann als
//! gesetzt gilt, wenn er in der Datenbank steht; der Schreib-Thread sammelt
//! Befehle zu Bündeln und sagt niemandem, wann einer durch ist. Im WAL-Modus
//! vertragen sich beide Schreiber, `busy_timeout` lässt den einen kurz warten.
//!
//! Diese Crate kennt die Audit-Kette nicht (`backlog/CONVENTIONS.md` 3.1). Sie
//! hält Nummer, Hash und Zeitpunkt; was sie bedeuten, weiß `humanitl-audit`,
//! und der Daemon verdrahtet beide.

use std::path::{Path, PathBuf};

use humanitl_core::Diagnostic;
use rusqlite::{Connection, ErrorCode, OptionalExtension as _, params};

use crate::error::{RecorderError, storage_failed};
use crate::schema;

/// Ein Anker, wie er in `audit_anchors` steht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditAnchor {
    /// Die Nummer des verankerten Records.
    pub seq: u64,
    /// Sein Hash als Hex.
    pub hash: String,
    /// Wann der Anker entstand, im Format des Audit-Logs.
    pub ts: String,
}

/// Eine eigene Schreibverbindung für die Anker.
#[derive(Debug)]
pub struct AnchorStore {
    conn: Connection,
    path: PathBuf,
}

impl AnchorStore {
    /// Öffnet die Datenbank der Aufzeichnung und bringt ihr Schema auf den
    /// neuesten Stand.
    ///
    /// # Errors
    ///
    /// `RECORDER_001`, wenn sich die Datenbank nicht öffnen oder migrieren
    /// lässt.
    pub fn open(db: &Path) -> Result<Self, Diagnostic> {
        let conn = schema::open_write(db).map_err(RecorderError::into_diagnostic)?;
        schema::migrate(&conn, db).map_err(RecorderError::into_diagnostic)?;
        schema::restrict(db).map_err(RecorderError::into_diagnostic)?;
        Ok(Self {
            conn,
            path: db.to_path_buf(),
        })
    }

    /// Die Datenbank.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Legt einen Anker ab.
    ///
    /// Derselbe Anker ein zweites Mal ist kein Fehler. Ein anderer Hash unter
    /// derselben Nummer schon: Dann tragen zwei Ketten dieselben Nummern, und
    /// der alte Anker bleibt stehen, denn er ist der Beleg.
    ///
    /// # Errors
    ///
    /// `RECORDER_003`, wenn die Zeile nicht geschrieben werden kann oder unter
    /// der Nummer schon ein anderer Hash steht.
    pub fn put(&self, anchor: &AuditAnchor) -> Result<(), Diagnostic> {
        let seq = sql_seq(anchor.seq)?;
        let inserted = self.conn.execute(
            "INSERT INTO audit_anchors (seq, hash, ts) VALUES (?1, ?2, ?3)",
            params![seq, anchor.hash, anchor.ts],
        );
        match inserted {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == ErrorCode::ConstraintViolation =>
            {
                let existing: Option<String> = self
                    .conn
                    .query_row(
                        "SELECT hash FROM audit_anchors WHERE seq = ?1",
                        params![seq],
                        |row| row.get(0),
                    )
                    .optional()
                    .map_err(|err| failed(&format!("cannot read anchor {} ({err})", anchor.seq)))?;
                if existing.as_deref() == Some(anchor.hash.as_str()) {
                    Ok(())
                } else {
                    Err(failed(&format!(
                        "audit_anchors already holds another hash for seq {}; the old anchor \
                         stays as evidence",
                        anchor.seq
                    )))
                }
            }
            Err(err) => Err(failed(&format!(
                "cannot store anchor {} ({err})",
                anchor.seq
            ))),
        }
    }

    /// Alle Anker, aufsteigend nach Nummer.
    ///
    /// # Errors
    ///
    /// `RECORDER_003`, wenn die Tabelle nicht lesbar ist.
    pub fn list(&self) -> Result<Vec<AuditAnchor>, Diagnostic> {
        list_on(&self.conn)
    }
}

/// Alle Anker aus `db`, nur lesend; eine fehlende Datenbank hat keine.
///
/// Für die Prüfung von außen, während oder nachdem der Daemon läuft.
///
/// # Errors
///
/// `RECORDER_001`, wenn sich die Datenbank nicht öffnen lässt, `RECORDER_003`,
/// wenn die Tabelle nicht lesbar ist.
pub fn read_anchors(db: &Path) -> Result<Vec<AuditAnchor>, Diagnostic> {
    if !db.exists() {
        return Ok(Vec::new());
    }
    let conn = schema::open_read(db).map_err(RecorderError::into_diagnostic)?;
    list_on(&conn)
}

fn list_on(conn: &Connection) -> Result<Vec<AuditAnchor>, Diagnostic> {
    let mut statement = conn
        .prepare("SELECT seq, hash, ts FROM audit_anchors ORDER BY seq")
        .map_err(|err| failed(&format!("cannot list the audit anchors ({err})")))?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(|err| failed(&format!("cannot read the audit anchors ({err})")))?;
    let mut anchors = Vec::new();
    for row in rows {
        let (seq, hash, ts) =
            row.map_err(|err| failed(&format!("cannot read an audit anchor ({err})")))?;
        let seq = u64::try_from(seq)
            .map_err(|_| failed(&format!("audit_anchors holds a negative seq ({seq})")))?;
        anchors.push(AuditAnchor { seq, hash, ts });
    }
    Ok(anchors)
}

fn sql_seq(seq: u64) -> Result<i64, Diagnostic> {
    i64::try_from(seq).map_err(|_| failed(&format!("seq {seq} does not fit into SQLite")))
}

fn failed(why: &str) -> Diagnostic {
    storage_failed(why.to_owned()).into_diagnostic()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{AnchorStore, AuditAnchor, read_anchors};

    fn anchor(seq: u64, hash: &str) -> AuditAnchor {
        AuditAnchor {
            seq,
            hash: hash.to_owned(),
            ts: "2026-09-11T10:00:00.000000Z".to_owned(),
        }
    }

    #[test]
    fn anchors_come_back_in_order_and_a_second_hash_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("humanitl.db");
        assert!(
            read_anchors(&db).unwrap().is_empty(),
            "no database, no anchors"
        );

        let store = AnchorStore::open(&db).unwrap();
        store.put(&anchor(6, "bb")).unwrap();
        store.put(&anchor(3, "aa")).unwrap();
        store.put(&anchor(3, "aa")).unwrap();
        let error = store.put(&anchor(3, "cc")).unwrap_err();
        assert_eq!(error.code.as_str(), "RECORDER_003");
        assert!(error.why.contains("seq 3"), "{}", error.why);

        let listed = read_anchors(&db).unwrap();
        assert_eq!(listed, vec![anchor(3, "aa"), anchor(6, "bb")]);
        assert_eq!(store.list().unwrap(), listed);
    }
}
