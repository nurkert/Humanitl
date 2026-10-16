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
//! welchen Fund sie gilt. Der Handler ruft sie an zwei Stellen: nach dem
//! ersten Scan, wo der Befund als
//! [`Flow::send_refusal`](humanitl_core::Flow::send_refusal) an den Flow
//! kommt, und nach dem zweiten Scan einer bearbeiteten Fassung.
//!
//! Seit HUM-159 wird eine solche Anfrage gehalten und nicht mehr sofort
//! geblockt, damit ein Mensch sie pseudonymisieren kann. Jeder Weg hinaus
//! prüft deshalb den Befund am Flow: Die Regel-Pipeline macht aus einer
//! Freigabe durch Regel oder Durchreiche [`hard_block_decision`], die
//! Warteschlange weist ein `Allow` zurück, und der Handler leitet ein `Allow`
//! mit gesetztem Befund nie weiter.

use humanitl_core::diagnostics::codes::HOLD_004;
use humanitl_core::{
    BlockReason, Decision, Diagnostic, Finding, FindingKind, FindingLocation, FixAction,
    HttpRequest, SecretMasks, Severity, Tier,
};
use humanitl_findings::{Detector, DetectorRegistry, FindingsSettings, ScanInput, ScanReport};

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
    // Der Satz gilt auf allen drei Wegen, die ihn bauen: an einer gehaltenen
    // Anfrage, deren `Allow` zurückgewiesen wird, an einer Regel-Freigabe, die
    // zur Sperre wird, und an einer bearbeiteten Fassung, deren Freigabe das
    // System zurücknimmt. Auf keinem davon ist etwas hinausgegangen.
    Err(Diagnostic::builder(HOLD_004, Severity::Blocking)
        .why(format!(
            "the request carries a checksum-confirmed {} in {place} and \
             hold.hard_block_checksum_secrets is on, so it cannot leave this machine unchanged; \
             nothing was sent",
            secret.kind.as_str(),
        ))
        .fix(FixAction::ChangeSetting {
            key: "hold.hard_block_checksum_secrets".to_owned(),
            value: "false".to_owned(),
        })
        .build())
}

/// Der Satz an den Agenten, wenn die harte Sperre eine Anfrage blockt.
///
/// Er nennt, was geschah, nie den Wert; der steht in keiner Meldung, nur sein
/// Hash. Er ist ein Satz der Maschine und landet deshalb nie in der Spalte,
/// die als Notiz eines Menschen gelesen wird (HUM-117).
pub const HARD_BLOCK_NOTE: &str = "a checksum-confirmed secret was found in this request and \
                                   hold.hard_block_checksum_secrets is on";

/// Die Sperre, die an die Stelle einer Freigabe tritt, wenn der Flow einen
/// [`Flow::send_refusal`](humanitl_core::Flow::send_refusal) trägt.
///
/// Der Grund heißt `secret` und nicht `user`: Es hat niemand so entschieden,
/// und eine Antwort, die einen Menschen nennt, den es nicht gab, wäre eine
/// Unwahrheit gegenüber dem Agenten und dem Protokoll
/// (`backlog/CONVENTIONS.md` 4.13).
#[must_use]
pub fn hard_block_decision() -> Decision {
    Decision::Block {
        reason: BlockReason::Secret,
        note: Some(HARD_BLOCK_NOTE.to_owned()),
    }
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
    /// Geheimnisse, die der Daemon selbst in den Verkehr gibt: Ein Fund mit
    /// genau diesem Wert fällt weg (HUM-140).
    masks: SecretMasks,
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
            masks: SecretMasks::default(),
        })
    }

    /// Derselbe Scanner mit dieser geteilten Liste bekannter Geheimnisse.
    ///
    /// Der Daemon reicht die Liste der Aufzeichnung herein
    /// (`Recorder::masks`). Zweierlei folgt daraus (HUM-140):
    ///
    /// - **Jedes Vorkommen ist ein Fund.** Der Detektor [`KNOWN_SECRET_DETECTOR`]
    ///   sucht jeden bekannten Wert wörtlich, auch in der Schreibweise einer
    ///   URL, in jeder Kopfzeile, in der Query und im entpackten Rumpf. Ein
    ///   Schlüssel ohne bekanntes Format (ein selbst gewählter für vLLM, einer
    ///   von Groq oder `OpenRouter`) fände sonst kein Muster, und der Agent
    ///   trüge ihn unbemerkt an jeden freigegebenen Host.
    /// - **Nur an seinem Ziel nicht.** In `Authorization` einer Anfrage an
    ///   das Ziel des Schlüssels fällt der Fund weg, sonst stünde an jeder
    ///   Anfrage an das Modell `LLM_005`.
    ///
    /// Der Detektor hängt auch dann, wenn `findings.enabled` aus ist: Er sucht
    /// keine Daten des Menschen, sondern das eine Geheimnis, das der Daemon
    /// selbst in die Sandbox gibt. Er hängt geschützt
    /// (`DetectorRegistry::register_protected`): Kein Begriff aus
    /// `findings.user_terms` verdrängt seinen Fund und damit die harte Sperre.
    #[must_use]
    pub fn with_masks(mut self, masks: SecretMasks) -> Self {
        self.registry.register_protected(Box::new(KnownSecrets {
            masks: masks.clone(),
        }));
        self.masks = masks;
        self
    }

    /// Ob `finding` der Zugangsschlüssel des Modells genau dort ist, wo er
    /// hingehört: in `Authorization` einer Anfrage an sein eigenes Ziel
    /// (HUM-140). Nur dann fällt der Fund weg.
    fn expected_key(&self, finding: &Finding, authority: &str) -> bool {
        matches!(
            &finding.location,
            FindingLocation::Header(name) if name.as_str() == "authorization"
        ) && self.masks.expected_at(&finding.value_hash, authority)
    }

    /// Die Kennungen der eingehängten Detektoren, für `humanitl doctor`.
    #[must_use]
    pub fn detector_ids(&self) -> Vec<&'static str> {
        self.registry.detector_ids()
    }
}

impl Scanner for Tier1Scanner {
    fn scan(&self, request: &HttpRequest, body: &[u8]) -> ScanReport {
        let mut report = self.registry.scan(request, body);
        let authority = request.authority.to_string();
        report
            .findings
            .retain(|finding| !self.expected_key(finding, &authority));
        report
    }

    /// Eine Notiz geht an den Agenten und nie an das Modell: Ein Schlüssel
    /// darin ist immer ein Fund.
    fn scan_note(&self, note: &str) -> Vec<Finding> {
        self.registry.scan_note(note)
    }
}

/// Die Kennung des Detektors für bekannte Geheimnisse, für `humanitl doctor`.
pub const KNOWN_SECRET_DETECTOR: &str = "known-secret";

/// Der Name, unter dem ein Fund den Zugangsschlüssel des Modells nennt.
pub const KNOWN_SECRET_NAME: &str = "llm";

/// Findet jedes bekannte Geheimnis aus [`SecretMasks`] wörtlich.
///
/// Ein Treffer ist ein exakter Vergleich mit einem Wert, den der Daemon kennt,
/// also sicherer als jede Prüfsumme: [`Tier::Checksum`]. Damit sperrt er unter
/// `hold.hard_block_checksum_secrets` hart wie jeder andere bestätigte
/// Schlüssel.
struct KnownSecrets {
    masks: SecretMasks,
}

impl Detector for KnownSecrets {
    fn id(&self) -> &'static str {
        KNOWN_SECRET_DETECTOR
    }

    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding> {
        self.masks
            .find_in(input.bytes)
            .into_iter()
            .filter_map(|span| {
                let value = std::str::from_utf8(input.bytes.get(span.clone())?).ok()?;
                Some(Finding::new(
                    FindingKind::ApiKey(KNOWN_SECRET_NAME.to_owned()),
                    span,
                    input.location.clone(),
                    Tier::Checksum,
                    value,
                ))
            })
            .collect()
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
        Finding, FindingKind, FindingLocation, FixAction, HeaderName, SecretMasks, Severity, Tier,
    };

    use super::{
        KNOWN_SECRET_NAME, ScanReport, Scanner as _, Tier1Scanner, check_allow, hard_blocks,
    };

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

    /// Ein Geheimnis in der Query heißt dort auch so, und der Satz stimmt auf
    /// allen drei Wegen, die ihn bauen (HUM-159): Er behauptet keine Sperre,
    /// die an einer gehaltenen Anfrage noch nicht geschehen ist, und sagt,
    /// dass nichts hinausging.
    #[test]
    fn the_refusal_names_the_query_and_says_nothing_was_sent() {
        let jwt = finding(FindingKind::Jwt, Tier::Checksum, FindingLocation::Query);
        let refused = check_allow(&[jwt], true).expect_err("a JWT is hard blocked");
        assert!(
            refused.why.contains(
                "jwt in the query and hold.hard_block_checksum_secrets is on, so it cannot \
                 leave this machine unchanged; nothing was sent"
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

    /// Der Zugangsschlüssel des Modells ist nur dort kein Fund, wo er
    /// hingehört: in `Authorization` einer Anfrage an sein eigenes Ziel. An
    /// einen anderen Host oder im Rumpf bleibt er einer (HUM-140, Review).
    #[test]
    fn a_known_key_is_no_finding_only_in_authorization_at_its_own_target() {
        const KEY: &str = "sk-proj-abcdefghij0123456789KLMN";
        let request = |host: &str, header: Option<&str>| {
            let mut request = humanitl_core::HttpRequest::new(
                humanitl_core::Method::POST,
                humanitl_core::Scheme::Http,
                humanitl_core::Authority::new(humanitl_core::HostName::parse(host).unwrap(), 11434),
                "/v1/chat/completions",
            );
            if let Some(token) = header {
                request.headers.insert(
                    HeaderName::from_static("authorization"),
                    humanitl_core::http::HeaderValue::from_str(&format!("Bearer {token}")).unwrap(),
                );
            }
            request
        };
        let masks = SecretMasks::default();
        let scanner = Tier1Scanner::new(&humanitl_findings::FindingsSettings::default())
            .unwrap()
            .with_masks(masks.clone());
        let own = "192.168.1.20";

        assert!(
            !scanner
                .scan(&request(own, Some(KEY)), &[])
                .findings
                .is_empty()
        );
        masks.add_for(KEY, Some("192.168.1.20:11434"));
        assert!(
            scanner
                .scan(&request(own, Some(KEY)), &[])
                .findings
                .is_empty(),
            "the key where it belongs is no finding"
        );
        assert!(
            !scanner
                .scan(&request("10.0.0.9", Some(KEY)), &[])
                .findings
                .is_empty(),
            "the key sent to another host stays a finding"
        );
        let body = format!("{{\"note\": \"{KEY}\"}}");
        assert!(
            !scanner
                .scan(&request(own, None), body.as_bytes())
                .findings
                .is_empty(),
            "the key in a body stays a finding, even at its own target"
        );
        assert!(
            !scanner
                .scan(
                    &request(own, Some("sk-proj-otherotherother0123456789")),
                    &[]
                )
                .findings
                .is_empty(),
            "another key in Authorization stays a finding"
        );
    }

    /// Ein Schlüssel ohne bekanntes Format findet kein Muster; der Detektor
    /// für bekannte Geheimnisse findet ihn trotzdem, im Rumpf, in einer
    /// eigenen Kopfzeile und in der Query, auch kodiert, an jeden Host. Nur
    /// `Authorization` an seinem Ziel bleibt frei (HUM-140, Review).
    #[test]
    fn a_known_key_without_a_known_format_is_found_wherever_it_leaves() {
        const KEY: &str = "local-key-0123456789abcdef";
        let request = |host: &str, path: &str| {
            humanitl_core::HttpRequest::new(
                humanitl_core::Method::POST,
                humanitl_core::Scheme::Https,
                humanitl_core::Authority::new(humanitl_core::HostName::parse(host).unwrap(), 443),
                path,
            )
        };
        let found_at = |report: &ScanReport| -> Vec<String> {
            report
                .findings
                .iter()
                .filter(|f| f.kind == FindingKind::ApiKey(KNOWN_SECRET_NAME.to_owned()))
                .map(|f| f.location.to_string())
                .collect()
        };
        let masks = SecretMasks::default();
        let scanner = Tier1Scanner::new(&humanitl_findings::FindingsSettings::default())
            .unwrap()
            .with_masks(masks.clone());
        let body = format!("{{\"prompt\": \"use {KEY}\"}}");
        let plain = request("allowed.example", "/upload");
        assert!(
            found_at(&scanner.scan(&plain, body.as_bytes())).is_empty(),
            "no pattern knows this format"
        );
        masks.add_for(KEY, Some("api.example.com:443"));

        assert_eq!(found_at(&scanner.scan(&plain, body.as_bytes())), ["body"]);

        let mut headed = request("allowed.example", "/upload");
        headed.headers.insert(
            HeaderName::from_static("x-api-key"),
            humanitl_core::http::HeaderValue::from_str(KEY).unwrap(),
        );
        assert_eq!(found_at(&scanner.scan(&headed, &[])), ["header:x-api-key"]);

        let queried = request("allowed.example", &format!("/v1?k={KEY}&x=1"));
        assert_eq!(found_at(&scanner.scan(&queried, &[])), ["query"]);

        let mut own = request("api.example.com", "/v1/chat/completions");
        own.headers.insert(
            HeaderName::from_static("authorization"),
            humanitl_core::http::HeaderValue::from_str(&format!("Bearer {KEY}")).unwrap(),
        );
        assert!(
            scanner.scan(&own, &[]).findings.is_empty(),
            "the key in Authorization at its own target is no finding"
        );
        own.headers.insert(
            HeaderName::from_static("x-api-key"),
            humanitl_core::http::HeaderValue::from_str(KEY).unwrap(),
        );
        assert_eq!(
            found_at(&scanner.scan(&own, &[])),
            ["header:x-api-key"],
            "at its own target, any other header is still a finding"
        );
    }

    /// Ein Begriff des Nutzers ist stärker als jede Prüfsumme und verdrängt
    /// beim Aufräumen schwächere Funde an derselben Stelle. Den Fund des
    /// Zugangsschlüssels darf er nicht verdrängen: `sk` aus einem
    /// Projekt-Profil hebelte sonst die harte Sperre für `sk-…` aus (HUM-140,
    /// Review).
    #[test]
    fn a_user_term_does_not_push_the_known_key_out_of_the_hard_block() {
        const KEY: &str = "sk-proj-abcdefghij0123456789KLMN";
        let settings = humanitl_findings::FindingsSettings::default().with_user_terms(["sk"]);
        let masks = SecretMasks::default();
        masks.add_for(KEY, Some("api.example.com:443"));
        let scanner = Tier1Scanner::new(&settings).unwrap().with_masks(masks);
        let mut request = humanitl_core::HttpRequest::new(
            humanitl_core::Method::POST,
            humanitl_core::Scheme::Https,
            humanitl_core::Authority::new(
                humanitl_core::HostName::parse("evil.example").unwrap(),
                443,
            ),
            "/collect",
        );
        request.headers.insert(
            HeaderName::from_static("authorization"),
            humanitl_core::http::HeaderValue::from_str(&format!("Bearer {KEY}")).unwrap(),
        );

        let findings = scanner.scan(&request, &[]).findings;
        assert!(
            findings
                .iter()
                .any(|f| f.kind == FindingKind::ApiKey(KNOWN_SECRET_NAME.to_owned())),
            "{findings:?}"
        );
        let refused = check_allow(&findings, true).expect_err("the hard block holds");
        assert_eq!(refused.code, humanitl_core::diagnostics::codes::HOLD_004);
    }

    /// Der bekannte Schlüssel steht an einer Stelle genau einmal, nicht als
    /// Muster und als bekannter Schlüssel nebeneinander; und eine Notiz an
    /// den Agenten, die ihn trägt, ist ein Fund (HUM-140, Review).
    #[test]
    fn a_known_key_is_found_once_per_place_and_in_a_note() {
        const KEY: &str = "sk-proj-abcdefghij0123456789KLMN";
        let masks = SecretMasks::default();
        masks.add_for(KEY, Some("api.example.com:443"));
        let scanner = Tier1Scanner::new(&humanitl_findings::FindingsSettings::default())
            .unwrap()
            .with_masks(masks);
        let mut request = humanitl_core::HttpRequest::new(
            humanitl_core::Method::POST,
            humanitl_core::Scheme::Https,
            humanitl_core::Authority::new(
                humanitl_core::HostName::parse("evil.example").unwrap(),
                443,
            ),
            "/collect",
        );
        request.headers.insert(
            HeaderName::from_static("authorization"),
            humanitl_core::http::HeaderValue::from_str(&format!("Bearer {KEY}")).unwrap(),
        );
        let findings = scanner.scan(&request, &[]).findings;
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(
            findings[0].kind,
            FindingKind::ApiKey(KNOWN_SECRET_NAME.to_owned())
        );

        let note = scanner.scan_note(&format!("please use {KEY} instead"));
        assert!(
            note.iter()
                .any(|f| f.kind == FindingKind::ApiKey(KNOWN_SECRET_NAME.to_owned())),
            "{note:?}"
        );
    }
}
