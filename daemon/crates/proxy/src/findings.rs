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
//! 1. **Kein Wert verlässt den Scan.** Ein [`Finding`]
//!    trägt Art, Ort, Bereich und Hash, nie den gefundenen Text. Auch das
//!    Protokoll bekommt ihn nicht: Wer einen Fund meldet, nennt `kind`,
//!    `location`, `span` und `value_hash`, sonst nichts.
//! 2. **Eine Lücke ist kein Freispruch.** Konnte nicht die ganze Anfrage
//!    durchsucht werden ([`ScanReport::truncated`]), muss das sichtbar
//!    bleiben; ein halb durchsuchter Body darf nie aussehen wie ein sauberer.
//! 3. **Der Scan scheitert nicht still.** Ist das eingebaute Regel-Set
//!    unbrauchbar, kommt [`Tier1Scanner::new`] gar nicht erst zustande
//!    (`FINDINGS_001`), und der Daemon startet nicht.
//!
//! Was aus einem Fund für die Freigabe folgt, steht ebenfalls hier und nicht
//! im Handler: [`check_allow`] ist die harte Sperre von
//! `hold.hard_block_checksum_secrets` (HUM-049), und [`hard_blocks`] sagt, für
//! welchen Fund sie gilt. Der Handler ruft beide an den zwei Stellen, an denen
//! eine Anfrage hinausgehen könnte: nach dem ersten Scan, bevor gefragt wird,
//! und nach dem zweiten Scan einer bearbeiteten Fassung.

use humanitl_core::diagnostics::codes::HOLD_004;
use humanitl_core::{
    Diagnostic, Finding, FindingKind, FindingLocation, FixAction, HttpRequest, Severity, Tier,
};
use humanitl_findings::{DetectorRegistry, FindingsSettings, ScanReport};

/// Ob `finding` unter `hold.hard_block_checksum_secrets` hart sperrt.
///
/// Nur ein Fund, den eine Prüfsumme bestätigt ([`Tier::Checksum`]), und nur in
/// den Arten, deren Verlust nicht zurückzuholen ist: API-Schlüssel, JWT, IBAN,
/// Kreditkarte (HUM-049). Ein Muster ohne Bestätigung sperrt nie hart:
/// Telefonnummern und IP-Adressen haben Fehlalarme, und eine Sperre, die
/// grundlos greift, lernt man zu umgehen.
#[must_use]
pub fn hard_blocks(finding: &Finding) -> bool {
    finding.tier == Tier::Checksum
        && matches!(
            finding.kind,
            FindingKind::ApiKey(_) | FindingKind::Jwt | FindingKind::Iban | FindingKind::CreditCard
        )
}

/// Die Prüfung vor jeder Freigabe: Darf eine Anfrage mit diesen Funden
/// hinaus?
///
/// `findings` sind die Funde der Anfrage, die hinausginge — bei einer
/// bearbeiteten die des zweiten Scans, nicht die der gehaltenen Fassung.
/// `hard_block` ist `hold.hard_block_checksum_secrets`. Ist der Schalter aus,
/// geht alles durch, was ein Mensch freigibt; ist er an, sperrt der erste Fund,
/// für den [`hard_blocks`] gilt. Eine Bestätigung aus der Oberfläche hebt die
/// Sperre nicht auf: Sie liegt hier, im Daemon, damit ein Client sie nicht
/// umgehen kann, und der einzige Weg an ihr vorbei ist, den Wert zu ersetzen
/// oder den Schalter umzulegen.
///
/// # Errors
///
/// [`HOLD_004`] mit Art und Ort des sperrenden Funds, nie mit seinem Wert,
/// und dem Vorschlag, den Schalter umzulegen.
pub fn check_allow(findings: &[Finding], hard_block: bool) -> Result<(), Diagnostic> {
    if !hard_block {
        return Ok(());
    }
    let Some(secret) = findings.iter().find(|finding| hard_blocks(finding)) else {
        return Ok(());
    };
    let place = match &secret.location {
        FindingLocation::Header(name) => format!("the {name} header"),
        FindingLocation::Query => "the query".to_owned(),
        FindingLocation::Body => "the body".to_owned(),
    };
    Err(Diagnostic::builder(HOLD_004, Severity::Blocking)
        .why(format!(
            "the request carries a checksum-confirmed {} in {place} and \
             hold.hard_block_checksum_secrets is on, so it was blocked and did not leave this \
             machine",
            secret.kind.as_str(),
        ))
        .fix(FixAction::ChangeSetting {
            key: "hold.hard_block_checksum_secrets".to_owned(),
            value: "false".to_owned(),
        })
        .build())
}

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

    /// Alles, was in einem Text des Menschen gefunden wurde.
    ///
    /// Für die Notiz einer Block-Entscheidung: Sie geht im Klartext an den
    /// Agenten, und ein Schlüssel darin verlässt damit den Rechner (HUM-117).
    /// Ein Fund daraus wird nie ein [`Finding`] am
    /// Fluss — die gehören zur Anfrage —, sondern genau ein Befund
    /// `FINDINGS_003` im Ereignisstrom. Regel 1 des Moduls gilt unverändert:
    /// Der Wert bleibt hier.
    fn scan_note(&self, note: &str) -> Vec<Finding>;
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

    fn scan_note(&self, note: &str) -> Vec<Finding> {
        self.registry.scan_note(note)
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

    fn scan_note(&self, _note: &str) -> Vec<Finding> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::{
        Finding, FindingKind, FindingLocation, FixAction, HeaderName, Severity, Tier,
    };

    use super::{check_allow, hard_blocks};

    fn finding(kind: FindingKind, tier: Tier, location: FindingLocation) -> Finding {
        Finding::new(kind, 0..8, location, tier, "value-never-shown")
    }

    fn iban() -> Finding {
        finding(FindingKind::Iban, Tier::Checksum, FindingLocation::Body)
    }

    /// Ein Muster ohne Prüfsumme sperrt nie hart, auch nicht mit Schalter:
    /// Telefonnummern und E-Mail-Adressen haben Fehlalarme.
    #[test]
    fn allow_with_open_regex_findings_ok() {
        let findings = [
            finding(FindingKind::Email, Tier::Regex, FindingLocation::Body),
            finding(FindingKind::Phone, Tier::Regex, FindingLocation::Query),
            finding(
                FindingKind::ApiKey("github".to_owned()),
                Tier::Regex,
                FindingLocation::Header(HeaderName::from_static("authorization")),
            ),
        ];
        assert_eq!(check_allow(&findings, true), Ok(()));
    }

    #[test]
    fn allow_with_checksum_secret_blocked_when_setting_on() {
        let findings = [
            finding(FindingKind::Email, Tier::Regex, FindingLocation::Body),
            iban(),
        ];
        let refused = check_allow(&findings, true).expect_err("an IBAN is hard blocked");
        assert_eq!(refused.code.as_str(), "HOLD_004");
        assert_eq!(refused.severity, Severity::Blocking);
        assert!(refused.why.contains("iban in the body"), "{}", refused.why);
        assert!(
            !refused.why.contains("value-never-shown"),
            "the value never reaches a message: {}",
            refused.why
        );
        assert_eq!(
            refused.fix,
            Some(FixAction::ChangeSetting {
                key: "hold.hard_block_checksum_secrets".to_owned(),
                value: "false".to_owned(),
            })
        );
    }

    /// Ohne Schalter gibt ein Mensch frei, was er will: Dieselbe IBAN geht.
    #[test]
    fn allow_ok_when_setting_off() {
        assert_eq!(check_allow(&[iban()], false), Ok(()));
    }

    /// Die vier Arten sperren mit Prüfsumme, keine andere, und ohne Prüfsumme
    /// keine von ihnen.
    #[test]
    fn only_checksum_secrets_of_the_four_kinds_hard_block() {
        for kind in [
            FindingKind::ApiKey("aws".to_owned()),
            FindingKind::Jwt,
            FindingKind::Iban,
            FindingKind::CreditCard,
        ] {
            let confirmed = finding(kind.clone(), Tier::Checksum, FindingLocation::Body);
            assert!(hard_blocks(&confirmed), "{kind:?} with a checksum");
            let guessed = finding(kind.clone(), Tier::Regex, FindingLocation::Body);
            assert!(!hard_blocks(&guessed), "{kind:?} without a checksum");
        }
        for kind in [
            FindingKind::Email,
            FindingKind::Phone,
            FindingKind::Ipv4,
            FindingKind::UserTerm("acme".to_owned()),
            FindingKind::Custom("x".to_owned()),
        ] {
            let confirmed = finding(kind.clone(), Tier::Checksum, FindingLocation::Body);
            assert!(!hard_blocks(&confirmed), "{kind:?} never hard blocks");
        }
    }

    /// Ein Geheimnis in der Query heißt dort auch so, und der Satz sagt, dass
    /// gesperrt wurde, nicht was noch zu tun wäre: Auf beiden Wegen, die ihn
    /// bauen, ist die Anfrage schon geblockt.
    #[test]
    fn the_refusal_names_the_query_and_says_it_was_blocked() {
        let jwt = finding(FindingKind::Jwt, Tier::Checksum, FindingLocation::Query);
        let refused = check_allow(&[jwt], true).expect_err("a JWT is hard blocked");
        assert!(
            refused.why.contains(
                "jwt in the query and hold.hard_block_checksum_secrets is on, so it was blocked"
            ),
            "{}",
            refused.why
        );
    }

    /// Der Ort steht im Satz so, wie ein Mensch ihn liest.
    #[test]
    fn the_refusal_names_a_header_by_its_name() {
        let card = finding(
            FindingKind::CreditCard,
            Tier::Checksum,
            FindingLocation::Header(HeaderName::from_static("x-card")),
        );
        let refused = check_allow(&[card], true).expect_err("a card is hard blocked");
        assert!(
            refused.why.contains("credit_card in the x-card header"),
            "{}",
            refused.why
        );
    }
}
