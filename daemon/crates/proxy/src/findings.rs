//! Der Anschluss für die Detektoren (HUM-025).
//!
//! Die Pipeline hat einen festen Platz für den Scan: nach dem Puffern des
//! Bodys, vor der Regel-Auswertung, damit
//! [`FlowEvent::Analyzed`](humanitl_core::FlowEvent) trägt, was gefunden
//! wurde, und der Mensch es sieht, bevor er entscheidet
//! (`backlog/sprint-2.md` HUM-023, Schritt 4).
//!
//! Der Proxy kennt die Detektoren nicht, er kennt diesen Port. `humanitld`
//! hängt den [`Tier1Scanner`] ein; Tests, die den Scan nicht brauchen, nehmen
//! [`NoScan`].
//!
//! Drei Regeln gelten für alles, was hier durchläuft:
//!
//! 1. **Kein Wert verlässt den Scan.** Ein [`Finding`](humanitl_core::Finding)
//!    trägt Art, Ort, Bereich und Hash, nie den gefundenen Text. Auch das
//!    Protokoll bekommt ihn nicht: Wer einen Fund meldet, nennt `kind`,
//!    `location`, `span` und `value_hash`, sonst nichts.
//! 2. **Eine Lücke ist kein Freispruch.** Konnte nicht die ganze Anfrage
//!    durchsucht werden ([`ScanReport::truncated`]), muss das sichtbar
//!    bleiben; ein halb durchsuchter Body darf nie aussehen wie ein sauberer.
//! 3. **Der Scan scheitert nicht still.** Ist das eingebaute Regel-Set
//!    unbrauchbar, kommt [`Tier1Scanner::new`] gar nicht erst zustande
//!    (`FINDINGS_001`), und der Daemon startet nicht.

use humanitl_core::{Diagnostic, HttpRequest};
use humanitl_findings::{DetectorRegistry, FindingsSettings, ScanReport};

/// Sucht in einer Anfrage nach Secrets und personenbezogenen Daten.
///
/// Der Aufruf ist synchron und darf nicht blockieren: Er läuft im Task der
/// Verbindung, zwischen dem letzten Byte des Bodys und der Entscheidung. Die
/// Laufzeit ist durch `limits.preview_cap_bytes` gedeckelt.
pub trait Scanner: Send + Sync {
    /// Alles, was in Kopfzeilen, Query und Body gefunden wurde, samt der
    /// Frage, ob überhaupt alles durchsucht werden konnte.
    ///
    /// `body` ist der gepufferte Request-Body; er ist durch
    /// `limits.hold_body_cap_bytes` gedeckelt.
    fn scan(&self, request: &HttpRequest, body: &[u8]) -> ScanReport;
}

/// Die Tier-1-Detektoren aus `humanitl-findings`.
#[derive(Debug)]
pub struct Tier1Scanner {
    registry: DetectorRegistry,
}

impl Tier1Scanner {
    /// Baut die Registry der Tier-1-Detektoren aus den Einstellungen.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, wenn das eingebaute Regel-Set unbrauchbar ist. Das ist
    /// ein Fehler im Daemon und beendet den Start: Eine Suche nach Geheimnissen
    /// darf nicht stillschweigend ausfallen, sonst hielte der Nutzer ein
    /// leeres Ergebnis für ein sauberes.
    pub fn new(settings: &FindingsSettings) -> Result<Self, Diagnostic> {
        Ok(Self {
            registry: DetectorRegistry::tier1(settings)?,
        })
    }

    /// Die Kennungen der eingehängten Detektoren, für `humanitl doctor`.
    #[must_use]
    pub fn detector_ids(&self) -> Vec<&'static str> {
        self.registry.detector_ids()
    }
}

impl Scanner for Tier1Scanner {
    fn scan(&self, request: &HttpRequest, body: &[u8]) -> ScanReport {
        self.registry.scan(request, body)
    }
}

/// Ein Scanner, der nichts sucht.
///
/// Für Tests, die den Weg einer Anfrage prüfen und nicht ihren Inhalt. Er
/// meldet ausdrücklich `truncated = false`: Es wurde nichts übersprungen, es
/// wurde nichts gesucht.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoScan;

impl Scanner for NoScan {
    fn scan(&self, _request: &HttpRequest, _body: &[u8]) -> ScanReport {
        ScanReport {
            findings: Vec::new(),
            truncated: false,
            diagnostics: Vec::new(),
        }
    }
}
