//! Die Prüfung einer Kette, die als Datei vorliegt (HUM-050, aus
//! `humanitl-audit` hierher gezogen mit HUM-155).
//!
//! Was geprüft wird und mit welchem Grund eine Kette bricht, steht in
//! `humanitl_audit::verify`; die Prüfung dort läuft über einen Leser. Hier
//! wird nur die Datei geöffnet, und eine, die sich nicht lesen lässt, wird
//! zu [`AUDIT_006`].

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;

use humanitl_audit::{Anchor, AuditVerifier, VerifyReport};
use humanitl_core::diagnostics::codes::AUDIT_006;
use humanitl_core::shell::shell_path;
use humanitl_core::{Diagnostic, FixAction, Severity};

/// Die Prüfung über eine Datei, als Funktionen von [`AuditVerifier`].
///
/// Ein Erweiterungs-Trait und kein Port: Er hat genau diese eine
/// Implementierung und hält nur den Aufruf `AuditVerifier::verify(path, ..)`
/// an seiner Stelle, seit die Datei nicht mehr in `humanitl-audit` geöffnet
/// wird (HUM-155).
pub trait VerifyFile {
    /// Prüft die Datei `path` gegen `anchors`, mit Schlüssel, wenn einer da ist.
    ///
    /// Eine fehlende Datei ist eine leere Kette; gibt es Anker, ist sie unter
    /// sie gekürzt.
    ///
    /// # Errors
    ///
    /// [`AUDIT_006`], wenn sich die Datei nicht lesen lässt. Eine Kette, die
    /// niemand lesen konnte, ist nicht geprüft, und ein `Ok` wäre gelogen.
    fn verify(
        path: &Path,
        hmac_key: Option<&[u8; 32]>,
        anchors: &[Anchor],
    ) -> Result<VerifyReport, Diagnostic> {
        Self::verify_until(path, hmac_key, anchors, None)
    }

    /// Wie [`VerifyFile::verify`], aber nur bis zum Record mit der Nummer
    /// `until`, dem Ende, das der Schreiber zuletzt gemeldet hat (HUM-156).
    ///
    /// Ein Daemon, der seine eigene, laufende Kette prüft, liest sie, während
    /// der Schreiber weiter anhängt. Was hinter `until` steht, ist noch nicht
    /// geschrieben und kein Bruch: Eine halbe Zeile dort ist eine, die gerade
    /// entsteht. Anker jenseits von `until` zählen aus demselben Grund nicht.
    /// `None` liest bis zum Ende der Datei.
    ///
    /// # Errors
    ///
    /// Wie [`VerifyFile::verify`].
    fn verify_until(
        path: &Path,
        hmac_key: Option<&[u8; 32]>,
        anchors: &[Anchor],
        until: Option<u64>,
    ) -> Result<VerifyReport, Diagnostic>;
}

impl VerifyFile for AuditVerifier {
    fn verify_until(
        path: &Path,
        hmac_key: Option<&[u8; 32]>,
        anchors: &[Anchor],
        until: Option<u64>,
    ) -> Result<VerifyReport, Diagnostic> {
        let result = match File::open(path) {
            Ok(file) => Self::verify_reader_until(BufReader::new(file), hmac_key, anchors, until),
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                Self::verify_reader_until(io::empty(), hmac_key, anchors, until)
            }
            Err(err) => Err(err),
        };
        result.map_err(|err| {
            Diagnostic::builder(AUDIT_006, Severity::Error)
                .why(format!(
                    "cannot read {} to verify it: {err}",
                    path.display()
                ))
                .fix(FixAction::CopyCommand(format!(
                    "ls -ln {}",
                    shell_path(path)
                )))
                .build()
        })
    }
}
