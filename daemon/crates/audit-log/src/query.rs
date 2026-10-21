//! Ende und Seiten der Kette aus der Datei (HUM-156, aus `humanitl-audit`
//! hierher gezogen mit HUM-155).
//!
//! Wie eine Seite entsteht und was sie beweist, steht in
//! `humanitl_audit::query`; die Funktionen dort lesen über einen Leser. Hier
//! wird nur die Datei geöffnet, eine fehlende ist eine leere Kette, und eine,
//! die sich nicht lesen lässt, wird zu [`AUDIT_006`].

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

pub use humanitl_audit::query::{
    DEFAULT_PAGE, Entry, MAX_PAGE, Page, QueryFilter, Tail, TimeRange,
};
use humanitl_audit::query::{query_from, tail_from};
use humanitl_core::diagnostics::codes::AUDIT_006;
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction, Severity};

/// So viel liest ein Aufruf von `read` auf einmal (HUM-163).
const READ_BUFFER: usize = 256 * 1024;

/// Liest eine Seite aus `path`.
///
/// `limit` null heißt [`DEFAULT_PAGE`], mehr als [`MAX_PAGE`] heißt
/// [`MAX_PAGE`]. `before` ist der Cursor: nur Records mit kleinerer Nummer.
/// `until` ist das gemeldete Ende (`humanitl_audit::query`). Eine fehlende
/// Datei ist eine leere Kette.
///
/// # Errors
///
/// [`AUDIT_006`], wenn sich die Datei nicht lesen lässt.
pub fn query(
    path: &Path,
    filter: &QueryFilter,
    limit: usize,
    before: Option<u64>,
    until: Option<u64>,
) -> Result<Page, Diagnostic> {
    query_from(|| open(path), filter, limit, before, until)
        .map_err(|err| unreadable(path, "list", &err))
}

/// Zählt die Records in `path` bis zum gemeldeten Ende `until` und liefert den
/// letzten.
///
/// # Errors
///
/// [`AUDIT_006`], wenn sich die Datei nicht lesen lässt.
pub fn tail(path: &Path, until: Option<u64>) -> Result<Tail, Diagnostic> {
    let doing = "read the head of";
    match open(path).map_err(|err| unreadable(path, doing, &err))? {
        Some(reader) => tail_from(reader, until).map_err(|err| unreadable(path, doing, &err)),
        None => Ok(Tail::default()),
    }
}

/// Ein Leser vom Anfang von `path`; `None`, wenn es die Datei nicht gibt.
fn open(path: &Path) -> io::Result<Option<BufReader<File>>> {
    match File::open(path) {
        Ok(file) => Ok(Some(BufReader::with_capacity(READ_BUFFER, file))),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err),
    }
}

/// Der Befund, wenn sich `path` nicht lesen lässt; `doing` sagt, wobei.
fn unreadable(path: &Path, doing: &str, err: &io::Error) -> Diagnostic {
    Diagnostic::builder(AUDIT_006, Severity::Error)
        .why(format!("cannot {doing} {}: {err}", path.display()))
        .fix(FixAction::CopyCommand(format!(
            "ls -ln {}",
            shell_path(path)
        )))
        .build()
}
