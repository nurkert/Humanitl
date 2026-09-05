//! Verbindungen und Migrationen.
//!
//! Die Migrationen liegen als `.sql` neben dem Code und werden einkompiliert;
//! ein Binary trägt sein Schema also immer bei sich. Der Stand steht in
//! `PRAGMA user_version`: das ist die Zählweise, die `SQLite` selbst dafür
//! vorsieht, und sie kommt ohne eine eigene Tabelle aus.
//!
//! Die Spezifikation (`backlog/sprint-2.md`, HUM-026) nennt `refinery`. Die
//! Crate steht nicht in `[workspace.dependencies]`, und diese Crate trägt keine
//! nackte Version ein (`backlog/CONVENTIONS.md` 4.11). Dateinamen und
//! Verzeichnis sind deshalb genau die von `refinery` erwarteten
//! (`migrations/V<n>__<name>.sql`); der Wechsel auf `refinery` ist später ein
//! Austausch dieses Moduls, keine Änderung am Schema.
//!
//! # Verbindungseinstellungen
//!
//! `journal_mode=WAL` muss vor der ersten Transaktion stehen, sonst blockiert
//! ein Schreibvorgang jeden Leser (`https://www.sqlite.org/wal.html`).
//! `synchronous=NORMAL` ist die für WAL empfohlene Stufe,
//! `foreign_keys=ON` hält `messages` und `findings` an ihren Flow gebunden, und
//! `busy_timeout=5000` lässt einen Leser warten statt `SQLITE_BUSY` zu melden.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use crate::error::{RecorderError, open_failed, open_failed_at, storage_failed};

/// Rechte der Datenbankdateien.
const FILE_MODE: u32 = 0o600;

/// Rechte des Datenverzeichnisses.
const DIR_MODE: u32 = 0o700;

/// So lange wartet eine Verbindung auf eine belegte Datenbank.
const BUSY_TIMEOUT_MS: u32 = 5_000;

/// Eine Migration, so wie sie in `migrations/` liegt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// Die Nummer aus dem Dateinamen, aufsteigend und lückenlos.
    pub version: u32,
    /// Der Name aus dem Dateinamen.
    pub name: &'static str,
    /// Das `SQL`.
    pub sql: &'static str,
}

/// Alle Migrationen in der Reihenfolge, in der sie laufen.
pub static MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("../migrations/V1__init.sql"),
    },
    Migration {
        version: 2,
        name: "rules_snapshot",
        sql: include_str!("../migrations/V2__rules_snapshot.sql"),
    },
    Migration {
        version: 3,
        name: "host_suffix",
        sql: include_str!("../migrations/V3__host_suffix.sql"),
    },
    Migration {
        version: 4,
        name: "flow_error",
        sql: include_str!("../migrations/V4__flow_error.sql"),
    },
    Migration {
        version: 5,
        name: "session_summary",
        sql: include_str!("../migrations/V5__session_summary.sql"),
    },
    Migration {
        version: 6,
        name: "meta_flow",
        sql: include_str!("../migrations/V6__meta_flow.sql"),
    },
];

/// Der Stand, den eine frisch migrierte Datenbank hat.
#[must_use]
pub fn latest_version() -> u32 {
    MIGRATIONS.last().map_or(0, |last| last.version)
}

/// Öffnet die Datenbank zum Schreiben und legt sie an, falls sie fehlt.
///
/// # Errors
///
/// [`RecorderError::Open`], wenn das Verzeichnis nicht anlegbar ist, die Datei
/// sich nicht öffnen lässt oder eine Einstellung nicht greift.
pub fn open_write(path: &Path) -> Result<Connection, RecorderError> {
    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            return Err(open_failed_at(
                parent,
                format!(
                    "could not create the data directory {} ({err})",
                    parent.display()
                ),
            ));
        }
        if let Err(err) = fs::set_permissions(parent, fs::Permissions::from_mode(DIR_MODE)) {
            return Err(open_failed_at(
                parent,
                format!(
                    "could not set the mode of {} to {DIR_MODE:o} ({err})",
                    parent.display()
                ),
            ));
        }
    }

    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(|err| {
        open_failed_at(
            path,
            format!("could not open the database {} ({err})", path.display()),
        )
    })?;
    configure(&conn, path, true)?;
    restrict(path)?;
    Ok(conn)
}

/// Öffnet die Datenbank nur zum Lesen.
///
/// Die Datei muss schon da sein; eine Leseverbindung legt keine an. Sie darf
/// die `-wal`- und `-shm`-Dateien trotzdem schreiben, sonst wäre WAL für Leser
/// nutzlos — `SQLITE_OPEN_READ_ONLY` betrifft die Datenbank, nicht ihren
/// Journal-Nachbarn.
///
/// # Errors
///
/// [`RecorderError::Open`], wenn sich die Datei nicht öffnen lässt oder eine
/// Einstellung nicht greift.
pub fn open_read(path: &Path) -> Result<Connection, RecorderError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(|err| {
        open_failed_at(
            path,
            format!(
                "could not open the database {} read-only ({err})",
                path.display()
            ),
        )
    })?;
    configure(&conn, path, false)?;
    Ok(conn)
}

/// Setzt die Einstellungen einer frisch geöffneten Verbindung.
fn configure(conn: &Connection, path: &Path, writable: bool) -> Result<(), RecorderError> {
    if writable {
        let mode: String = conn
            .query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))
            .map_err(|err| {
                open_failed(
                    format!(
                        "could not switch {} to journal_mode=WAL ({err})",
                        path.display()
                    ),
                    None,
                )
            })?;
        if !mode.eq_ignore_ascii_case("wal") {
            return Err(open_failed(
                format!(
                    "{} stayed in journal_mode={mode}; WAL is required so that readers are not \
                     blocked while a flow is written",
                    path.display()
                ),
                None,
            ));
        }
        pragma(conn, path, "synchronous", "NORMAL")?;
    }
    pragma(conn, path, "foreign_keys", "ON")?;
    conn.busy_timeout(std::time::Duration::from_millis(u64::from(BUSY_TIMEOUT_MS)))
        .map_err(|err| {
            open_failed(
                format!("could not set busy_timeout on {} ({err})", path.display()),
                None,
            )
        })?;
    Ok(())
}

/// Setzt ein `PRAGMA`, das keine Zeile zurückgibt.
fn pragma(conn: &Connection, path: &Path, name: &str, value: &str) -> Result<(), RecorderError> {
    conn.execute_batch(&format!("PRAGMA {name}={value};"))
        .map_err(|err| {
            open_failed(
                format!("could not set {name}={value} on {} ({err})", path.display()),
                None,
            )
        })
}

/// Setzt die Rechte der Datenbankdateien auf `0600`.
///
/// `-wal` und `-shm` entstehen erst mit der ersten Transaktion, also nicht
/// beim Öffnen. Sie tragen den frischesten Teil der Aufzeichnung — Kopfzeilen,
/// Bodies, alles, was noch nicht in die Hauptdatei geschrieben wurde — und
/// entstehen mit der Standardmaske des Prozesses, üblicherweise `0644`. Wer
/// eine Transaktion auslöst, ruft deshalb danach diese Funktion: einmal nach
/// den Migrationen und einmal im Schreib-Thread nach dessen erstem Commit.
/// Fehlt eine der Dateien noch, wird sie übergangen.
///
/// # Errors
///
/// [`RecorderError::Open`], wenn eine vorhandene Datei sich nicht auf `0600`
/// setzen lässt.
pub fn restrict(path: &Path) -> Result<(), RecorderError> {
    for candidate in [
        path.to_path_buf(),
        sibling(path, "-wal"),
        sibling(path, "-shm"),
    ] {
        if !candidate.is_file() {
            continue;
        }
        if let Err(err) = fs::set_permissions(&candidate, fs::Permissions::from_mode(FILE_MODE)) {
            return Err(open_failed_at(
                &candidate,
                format!(
                    "could not set the mode of {} to {FILE_MODE:o} ({err})",
                    candidate.display()
                ),
            ));
        }
    }
    Ok(())
}

/// Der Pfad der Datenbank mit einem Anhängsel: `humanitl.db-wal`.
fn sibling(path: &Path, suffix: &str) -> std::path::PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(suffix);
    std::path::PathBuf::from(name)
}

/// Bringt die Datenbank auf den neuesten Stand und liefert ihn zurück.
///
/// Jede Migration läuft in einer eigenen Transaktion zusammen mit dem
/// Fortschreiben von `user_version`: entweder beide oder keines von beiden.
///
/// # Errors
///
/// [`RecorderError::Open`], wenn der Stand nicht lesbar ist, eine Migration
/// scheitert oder die Datenbank neuer ist als dieses Binary.
pub fn migrate(conn: &Connection, path: &Path) -> Result<u32, RecorderError> {
    let current: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(|err| {
            open_failed(
                format!("could not read user_version of {} ({err})", path.display()),
                None,
            )
        })?;
    let latest = latest_version();
    if current > latest {
        return Err(open_failed(
            format!(
                "{} was written by a newer Humanitl (schema {current}, this build knows \
                 {latest}); install the newer version or point recorder at another file",
                path.display()
            ),
            None,
        ));
    }

    for migration in MIGRATIONS {
        if migration.version <= current {
            continue;
        }
        let script = format!(
            "BEGIN;\n{}\nPRAGMA user_version = {};\nCOMMIT;",
            migration.sql, migration.version
        );
        if let Err(err) = conn.execute_batch(&script) {
            let _ignored = conn.execute_batch("ROLLBACK;");
            return Err(open_failed_at(
                path,
                format!(
                    "migration V{}__{} failed on {} ({err})",
                    migration.version,
                    migration.name,
                    path.display()
                ),
            ));
        }
    }
    Ok(latest)
}

/// Trägt `flows.host_rev` für Zeilen nach, die vor der Migration V3 entstanden.
///
/// Die Migration kann die Spalte nur mit einem festen Wert füllen; das Umdrehen
/// der Labels ist Rust-Code ([`crate::hostkey::host_key`]). Gerechnet wird je
/// verschiedenem Host, nicht je Zeile, und nur solange es Zeilen mit leerem
/// Schlüssel gibt — beim zweiten Start ist die Schleife leer.
///
/// # Errors
///
/// [`RecorderError::Storage`], wenn das Nachtragen scheitert.
pub fn backfill_host_rev(conn: &Connection) -> Result<u64, RecorderError> {
    let hosts: Vec<String> = {
        let mut statement = conn
            .prepare("SELECT DISTINCT host FROM flows WHERE host_rev = \'\'")
            .map_err(|err| {
                storage_failed(format!("could not list the hosts without a key ({err})"))
            })?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|err| storage_failed(format!("could not read the hosts ({err})")))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row.map_err(|err| storage_failed(format!("could not read a host ({err})")))?);
        }
        out
    };

    let mut touched = 0;
    for host in hosts {
        let rows = conn
            .execute(
                "UPDATE flows SET host_rev = ?2 WHERE host = ?1 AND host_rev = \'\'",
                rusqlite::params![host, crate::hostkey::host_key(&host)],
            )
            .map_err(|err| {
                storage_failed(format!("could not backfill host_rev for {host} ({err})"))
            })?;
        touched += u64::try_from(rows).unwrap_or(0);
    }
    Ok(touched)
}

/// Der Journal-Modus, in dem eine Verbindung gerade steht.
///
/// # Errors
///
/// [`RecorderError::Storage`], wenn sich das `PRAGMA` nicht lesen lässt.
pub fn journal_mode(conn: &Connection) -> Result<String, RecorderError> {
    conn.query_row("PRAGMA journal_mode", [], |row| row.get(0))
        .map_err(|err| storage_failed(format!("could not read journal_mode ({err})")))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{MIGRATIONS, journal_mode, latest_version, migrate, open_read, open_write};

    #[test]
    fn migrations_are_numbered_without_gaps() {
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            let expected = u32::try_from(index).unwrap_or(u32::MAX) + 1;
            assert_eq!(migration.version, expected, "migration order");
            assert!(!migration.sql.trim().is_empty());
        }
    }

    #[test]
    fn migrating_twice_changes_nothing() {
        let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
        let path = dir.path().join("humanitl.db");
        let conn = open_write(&path).unwrap_or_else(|err| panic!("{err}"));
        assert_eq!(
            migrate(&conn, &path).unwrap_or_else(|err| panic!("{err}")),
            latest_version()
        );
        assert_eq!(
            migrate(&conn, &path).unwrap_or_else(|err| panic!("{err}")),
            latest_version()
        );
        assert_eq!(
            journal_mode(&conn).unwrap_or_default().to_ascii_lowercase(),
            "wal"
        );

        let reader = open_read(&path).unwrap_or_else(|err| panic!("{err}"));
        let tables: i64 = reader
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN \
                 ('sessions', 'flows', 'messages', 'findings', 'rules_snapshot', \
                 'session_summaries')",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(tables, 6);
    }

    #[test]
    fn a_newer_schema_is_refused() {
        let dir = tempfile::tempdir().unwrap_or_else(|err| panic!("{err}"));
        let path = dir.path().join("humanitl.db");
        let conn = open_write(&path).unwrap_or_else(|err| panic!("{err}"));
        conn.execute_batch("PRAGMA user_version = 99;")
            .unwrap_or_else(|err| panic!("{err}"));
        let err = migrate(&conn, &path)
            .err()
            .unwrap_or_else(|| panic!("no error"));
        assert_eq!(err.diagnostic().code.as_str(), "RECORDER_001");
    }
}
