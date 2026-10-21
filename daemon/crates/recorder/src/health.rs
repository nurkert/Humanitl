//! Ob die Aufzeichnung gerade schreibt, so wie die Clients es erfahren
//! (HUM-250).
//!
//! Ein Schreibfehler kommt selten allein: Ist die Platte voll oder die
//! Datenbank gesperrt, scheitert jede Transaktion, und ein Befund je Zeile
//! begrübe den Ereignisstrom unter Dutzenden gleicher Karten, während der
//! eine Satz, auf den es ankommt, darin untergeht. Dieser Melder fasst
//! deshalb zusammen:
//!
//! * **Einmal je Ursache.** Ein Befund, der die Aufzeichnung als gestört
//!   meldet (`RECORDER_001`, die Datenbank ist nicht verfügbar;
//!   `RECORDER_003`, ein Schreibvorgang scheiterte), geht beim ersten Mal in
//!   den Strom. Jeder weitere desselben Codes steht nur noch im Protokoll,
//!   bis die Störung vorbei ist.
//! * **Ein zweiter Befund beim Ende.** Schreibt der Thread danach eine
//!   Transaktion ohne jeden Fehler, geht `RECORDER_005` hinaus, und die
//!   Oberfläche nimmt ihre Hinweise zurück. Was dazwischen lag, fehlt in der
//!   History; nachgetragen wird es nicht.
//! * **Der Stand für späte Zuhörer.** Ein Client, der sich während der
//!   Störung verbindet, hat den ersten Befund verpasst. [`Reporter::outage`]
//!   nennt die Befunde der laufenden Störung, damit der Dienst sie einem
//!   neuen Abonnenten vor allem anderen nachliefert.
//!
//! Alles andere, etwa eine Warnung über die Rechte von `-wal` oder ein Body,
//! der nicht in den Blob-Speicher passte, geht unverändert durch: Es sagt
//! nichts darüber, ob die Aufzeichnung als Ganzes läuft.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::SystemTime;

use humanitl_core::diagnostics::codes::{RECORDER_001, RECORDER_003, RECORDER_005};
use humanitl_core::{Diagnostic, DiagnosticCode, Severity};
use tokio::sync::broadcast;

/// Ein Befund der laufenden Störung und wann er zuerst auftrat.
pub type OutageEntry = (SystemTime, Diagnostic);

/// Der geteilte Melder des Schreib-Threads, des Handles und jedes
/// Mitschnitts.
#[derive(Debug, Clone)]
pub struct Reporter {
    tx: broadcast::Sender<Diagnostic>,
    outage: Arc<Mutex<Vec<OutageEntry>>>,
}

impl Reporter {
    /// Ein Melder, der auf `tx` sendet.
    pub fn new(tx: broadcast::Sender<Diagnostic>) -> Self {
        Self {
            tx,
            outage: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Ein neuer Zuhörer.
    pub fn subscribe(&self) -> broadcast::Receiver<Diagnostic> {
        self.tx.subscribe()
    }

    /// Meldet einen Befund; wahr, wenn er in den Strom ging.
    ///
    /// Ein Befund, der die Aufzeichnung als gestört meldet
    /// ([`is_outage`]), geht nur beim ersten Mal seines Codes in dieser
    /// Störung hinaus. Jeder andere geht immer.
    pub fn report(&self, diagnostic: Diagnostic) -> bool {
        if is_outage(&diagnostic) {
            let mut outage = self.outage.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(entry) = outage
                .iter_mut()
                .find(|(_, seen)| seen.code == diagnostic.code)
            {
                // Derselbe Code mit einem Weg hinaus, wo der erste keinen
                // hatte: ein toter Schreib-Thread nach einem Schreibfehler.
                // Sein „Daemon neu starten“ ersetzt den ersten Befund, damit
                // ihn auch ein später Client sieht.
                if diagnostic.fix.is_none() || entry.1.fix.is_some() {
                    return false;
                }
                entry.1 = diagnostic.clone();
            } else {
                outage.push((SystemTime::now(), diagnostic.clone()));
            }
        }
        let _ignored = self.tx.send(diagnostic);
        true
    }

    /// Eine Transaktion ging ohne jeden Fehler durch.
    ///
    /// War die Aufzeichnung als gestört gemeldet, endet die Störung hier, und
    /// `RECORDER_005` geht hinaus; sonst geschieht nichts.
    pub fn recovered(&self) {
        let ended = {
            let mut outage = self.outage.lock().unwrap_or_else(PoisonError::into_inner);
            let ended = !outage.is_empty();
            outage.clear();
            ended
        };
        if ended {
            let diagnostic = Diagnostic::builder(RECORDER_005, Severity::Info)
                .why(
                    "the recording writes again; requests decided while it was disrupted are \
                     missing from the history"
                        .to_owned(),
                )
                .build();
            tracing::info!(code = diagnostic.code.as_str(), "recorder");
            let _ignored = self.tx.send(diagnostic);
        }
    }

    /// Die Befunde der laufenden Störung, ältester zuerst; leer, solange die
    /// Aufzeichnung schreibt.
    pub fn outage(&self) -> Vec<OutageEntry> {
        self.outage
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

/// Die Codes, mit denen die Aufzeichnung als Ganzes gestört ist.
const OUTAGE_CODES: [DiagnosticCode; 2] = [RECORDER_001, RECORDER_003];

/// Wahr, wenn [`Diagnostic`] die Aufzeichnung als gestört meldet: einer der
/// beiden Codes, und mindestens `Error`. Eine Warnung desselben Codes, etwa
/// über die Rechte einer Datei, lässt die Aufzeichnung laufen.
#[must_use]
pub fn is_outage(diagnostic: &Diagnostic) -> bool {
    OUTAGE_CODES.contains(&diagnostic.code) && diagnostic.severity >= Severity::Error
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::diagnostics::codes::RECORDER_004;

    use super::*;

    fn diagnostic(code: DiagnosticCode, severity: Severity) -> Diagnostic {
        Diagnostic::builder(code, severity)
            .why("test".to_owned())
            .build()
    }

    fn drain(rx: &mut broadcast::Receiver<Diagnostic>) -> Vec<&'static str> {
        let mut codes = Vec::new();
        while let Ok(diagnostic) = rx.try_recv() {
            codes.push(diagnostic.code.as_str());
        }
        codes
    }

    /// Dreimal derselbe Schreibfehler, einmal ein Fehler beim Öffnen: zwei
    /// Befunde, einer je Ursache. Danach das Ende der Störung, und ein neuer
    /// Fehler meldet sich wieder.
    #[test]
    fn once_per_cause_until_recovery() {
        let (tx, mut rx) = broadcast::channel(16);
        let reporter = Reporter::new(tx);

        assert!(reporter.report(diagnostic(RECORDER_003, Severity::Error)));
        assert!(!reporter.report(diagnostic(RECORDER_003, Severity::Error)));
        assert!(!reporter.report(diagnostic(RECORDER_003, Severity::Error)));
        assert!(reporter.report(diagnostic(RECORDER_001, Severity::Error)));
        assert_eq!(drain(&mut rx), ["RECORDER_003", "RECORDER_001"]);
        assert_eq!(reporter.outage().len(), 2);

        reporter.recovered();
        assert_eq!(drain(&mut rx), ["RECORDER_005"]);
        assert!(reporter.outage().is_empty());

        // Ohne Störung sagt ein gelungener Commit nichts.
        reporter.recovered();
        assert!(drain(&mut rx).is_empty());

        assert!(reporter.report(diagnostic(RECORDER_003, Severity::Error)));
        assert_eq!(drain(&mut rx), ["RECORDER_003"]);
    }

    /// Stirbt der Schreib-Thread nach einem Schreibfehler, ersetzt sein
    /// Befund mit dem Fix „neu starten“ den ersten, einmal (Review).
    #[test]
    fn a_dead_writer_replaces_the_earlier_write_error() {
        let (tx, mut rx) = broadcast::channel(16);
        let reporter = Reporter::new(tx);
        let gone = || {
            Diagnostic::builder(RECORDER_003, Severity::Error)
                .why("the recorder thread is gone".to_owned())
                .fix(humanitl_core::FixAction::CopyCommand(
                    "systemctl --user restart humanitld".to_owned(),
                ))
                .build()
        };

        assert!(reporter.report(diagnostic(RECORDER_003, Severity::Error)));
        assert!(reporter.report(gone()));
        assert!(!reporter.report(gone()));
        assert!(!reporter.report(diagnostic(RECORDER_003, Severity::Error)));
        assert_eq!(drain(&mut rx), ["RECORDER_003", "RECORDER_003"]);
        let outage = reporter.outage();
        assert_eq!(outage.len(), 1);
        assert!(outage[0].1.fix.is_some(), "{:?}", outage[0].1);
    }

    /// Was die Aufzeichnung nicht als Ganzes stört, geht jedes Mal durch und
    /// beginnt keine Störung.
    #[test]
    fn other_findings_pass_unchanged() {
        let (tx, mut rx) = broadcast::channel(16);
        let reporter = Reporter::new(tx);

        assert!(reporter.report(diagnostic(RECORDER_004, Severity::Error)));
        assert!(reporter.report(diagnostic(RECORDER_004, Severity::Error)));
        assert!(reporter.report(diagnostic(RECORDER_001, Severity::Warning)));
        assert!(reporter.report(diagnostic(RECORDER_001, Severity::Warning)));
        assert_eq!(
            drain(&mut rx),
            [
                "RECORDER_004",
                "RECORDER_004",
                "RECORDER_001",
                "RECORDER_001"
            ]
        );
        assert!(reporter.outage().is_empty());
        reporter.recovered();
        assert!(drain(&mut rx).is_empty());
    }
}
