//! Der Fehlertyp der Aufzeichnung.
//!
//! Jeder Fehlerpfad trägt ein [`Diagnostic`] mit `why` und, wo es eine gibt,
//! einer Behebung (ADR-012). `RecorderError` ist nur die Hülle, die sagt,
//! welcher Teil der Aufzeichnung stolperte: der Filter des Aufrufers, die
//! Datenbank oder der Blob-Speicher.

use humanitl_core::diagnostics::codes::{RECORDER_001, RECORDER_002, RECORDER_003, RECORDER_004};
use humanitl_core::{Diagnostic, FixAction, Severity};

/// Was in der Aufzeichnung schiefging.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum RecorderError {
    /// Die Aufzeichnung ließ sich nicht öffnen (`RECORDER_001`).
    #[error(transparent)]
    Open(Diagnostic),
    /// Der Filterausdruck ließ sich nicht lesen (`RECORDER_002`).
    #[error(transparent)]
    Filter(Diagnostic),
    /// Die Datenbank hat einen Zugriff abgelehnt (`RECORDER_003`).
    #[error(transparent)]
    Storage(Diagnostic),
    /// Der Blob-Speicher ließ sich nicht lesen oder schreiben (`RECORDER_004`).
    #[error(transparent)]
    Blob(Diagnostic),
}

impl RecorderError {
    /// Der Befund hinter dem Fehler.
    #[must_use]
    pub const fn diagnostic(&self) -> &Diagnostic {
        match self {
            Self::Open(diagnostic)
            | Self::Filter(diagnostic)
            | Self::Storage(diagnostic)
            | Self::Blob(diagnostic) => diagnostic,
        }
    }

    /// Der Befund hinter dem Fehler, übernommen.
    #[must_use]
    pub fn into_diagnostic(self) -> Diagnostic {
        match self {
            Self::Open(diagnostic)
            | Self::Filter(diagnostic)
            | Self::Storage(diagnostic)
            | Self::Blob(diagnostic) => diagnostic,
        }
    }

    /// Hängt einen Behebungsvorschlag an, falls noch keiner dran ist.
    ///
    /// Ein Befund ohne `fix` ist die Ausnahme, nicht die Regel
    /// (`backlog/CONVENTIONS.md` 4.13). Wo der Aufrufer den konkreten Pfad
    /// kennt, hängt er ihn hier an, statt den Text zu wiederholen.
    #[must_use]
    pub fn with_fix(self, fix: FixAction) -> Self {
        let wrap: fn(Diagnostic) -> Self = match self {
            Self::Open(_) => Self::Open,
            Self::Filter(_) => Self::Filter,
            Self::Storage(_) => Self::Storage,
            Self::Blob(_) => Self::Blob,
        };
        let mut diagnostic = self.into_diagnostic();
        if diagnostic.fix.is_none() {
            diagnostic.fix = Some(fix);
        }
        wrap(diagnostic)
    }
}

/// Der Vorschlag, sich einen Pfad anzusehen: Rechte, Eigentümer, freier Platz.
///
/// Die drei Fragen, die hinter fast jedem Fehler des Dateisystems stehen, als
/// ein Befehl, den die Oberfläche als Knopf und die Kommandozeile als Zeile
/// zeigt.
#[must_use]
pub fn inspect_path(path: &std::path::Path) -> FixAction {
    let shown = path.display();
    FixAction::CopyCommand(format!("ls -ld {shown} && df -h {shown}"))
}

/// `RECORDER_001`: die Aufzeichnung ließ sich nicht öffnen.
///
/// Blockierend, weil ohne Aufzeichnung die Zusage aus `README.md` („alles wird
/// aufgezeichnet") nicht mehr gilt; der Daemon startet dann nicht weiter, statt
/// unbemerkt ohne Gedächtnis zu laufen.
pub(crate) fn open_failed(why: impl Into<String>, fix: Option<FixAction>) -> RecorderError {
    let mut builder = Diagnostic::builder(RECORDER_001, Severity::Blocking).why(why);
    if let Some(fix) = fix {
        builder = builder.fix(fix);
    }
    RecorderError::Open(builder.build())
}

/// `RECORDER_001` mit dem Pfad, den man sich ansehen sollte.
pub(crate) fn open_failed_at(path: &std::path::Path, why: impl Into<String>) -> RecorderError {
    open_failed(why, Some(inspect_path(path)))
}

/// `RECORDER_002`: der Filterausdruck ließ sich nicht lesen.
pub(crate) fn filter_failed(why: impl Into<String>) -> RecorderError {
    RecorderError::Filter(
        Diagnostic::builder(RECORDER_002, Severity::Error)
            .why(why)
            .build(),
    )
}

/// `RECORDER_002`: der Cursor passt nicht zur Sortierung.
///
/// Derselbe Code wie der Filter, weil es dieselbe Art Fehler ist: die Anfrage
/// des Aufrufers ist so nicht ausführbar.
pub(crate) fn cursor_mismatch(why: impl Into<String>) -> RecorderError {
    RecorderError::Filter(
        Diagnostic::builder(RECORDER_002, Severity::Error)
            .why(why)
            .title("Cursor passt nicht zur Sortierung")
            .build(),
    )
}

/// `RECORDER_003`: ein Zugriff auf die Datenbank schlug fehl.
///
/// `why` nennt den Vorgang und die Kennungen, nie den Inhalt einer Nachricht:
/// Bodies verlassen die Datenbank nicht über einen Fehlertext.
pub(crate) fn storage_failed(why: impl Into<String>) -> RecorderError {
    RecorderError::Storage(
        Diagnostic::builder(RECORDER_003, Severity::Error)
            .why(why)
            .build(),
    )
}

/// `RECORDER_004`: der Blob-Speicher ließ sich nicht lesen oder schreiben.
pub(crate) fn blob_failed(why: impl Into<String>) -> RecorderError {
    RecorderError::Blob(
        Diagnostic::builder(RECORDER_004, Severity::Error)
            .why(why)
            .build(),
    )
}

/// `RECORDER_004` mit dem Pfad, den man sich ansehen sollte.
pub(crate) fn blob_failed_at(path: &std::path::Path, why: impl Into<String>) -> RecorderError {
    blob_failed(why).with_fix(inspect_path(path))
}
