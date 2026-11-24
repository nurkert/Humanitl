//! Der `Detector`-Trait und die Registry, die alle Detektoren zusammenhält.
//!
//! Die Registry ist der Erweiterungspunkt aus `BACKLOG.md` Abschnitt 6: ein
//! Trait, eine Registry, im MVP genau die sieben eingebauten Detektoren. Ein
//! späterer Detektor aus einem Plugin wird mit [`DetectorRegistry::register`]
//! dazugelegt und braucht keine Änderung an dieser Datei.

use core::ops::Range;
use std::collections::HashSet;

use humanitl_core::http::HttpRequest;
use humanitl_core::{Diagnostic, Finding, FindingLocation, Tier};

use crate::detectors;
use crate::input::{ScanInput, ScanTargets};
use crate::settings::FindingsSettings;

/// Die Kennungen der Tier-1-Detektoren, in der Reihenfolge, in der sie laufen.
pub const TIER1_DETECTOR_IDS: [&str; 7] = [
    "secrets",
    "email",
    "iban",
    "card",
    "phone",
    "ipv4",
    "user_terms",
];

/// Ein Detektor sucht in einem Suchziel nach genau einer Art von Wert.
///
/// Die Bereiche der zurückgegebenen Funde sind relativ zu `input.bytes`. Ein
/// Detektor hält keinen Zustand über den Aufruf hinaus und darf nichts tun als
/// rechnen: kein IO, keine Uhr, kein Zufall.
pub trait Detector: Send + Sync {
    /// Die Kennung, wie sie in `findings.enabled` steht.
    fn id(&self) -> &'static str;

    /// Sucht in diesem Suchziel.
    fn scan(&self, input: &ScanInput<'_>) -> Vec<Finding>;
}

/// Das Ergebnis eines Scans über eine ganze Anfrage.
#[derive(Debug, Clone, PartialEq)]
pub struct ScanReport {
    /// Die Funde, dedupliziert und sortiert.
    pub findings: Vec<Finding>,
    /// Wahr, wenn nicht die ganze Anfrage durchsucht wurde.
    pub truncated: bool,
    /// Die Befunde, die eine Lücke erklären (`FINDINGS_002`).
    pub diagnostics: Vec<Diagnostic>,
}

/// Alle Detektoren, die auf eine Anfrage losgelassen werden.
pub struct DetectorRegistry {
    detectors: Vec<Box<dyn Detector>>,
    /// Detektoren, deren Funde das Aufräumen überstehen
    /// ([`DetectorRegistry::register_protected`]).
    protected: Vec<Box<dyn Detector>>,
    ignored: HashSet<[u8; 32]>,
    cap_bytes: usize,
    max_decompress_ratio: u32,
}

impl core::fmt::Debug for DetectorRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DetectorRegistry")
            .field("detectors", &self.detector_ids())
            .field("protected", &self.protected.len())
            .field("ignored", &self.ignored.len())
            .field("cap_bytes", &self.cap_bytes)
            .field("max_decompress_ratio", &self.max_decompress_ratio)
            .finish()
    }
}

impl DetectorRegistry {
    /// Baut die Registry mit den Tier-1-Detektoren.
    ///
    /// Ist `findings.enabled` aus, bleibt die Registry leer: kein Detektor
    /// läuft, und [`DetectorRegistry::scan_request`] liefert nichts.
    ///
    /// # Errors
    ///
    /// `FINDINGS_001`, wenn das eingebaute Regel-Set der Secret-Detektoren
    /// nicht lesbar ist oder eines seiner Muster sich nicht übersetzen lässt.
    /// Das ist ein Fehler im Daemon; die Suche wird nicht stillschweigend
    /// übersprungen.
    pub fn tier1(settings: &FindingsSettings) -> Result<Self, Diagnostic> {
        let mut registry = Self::empty(settings);
        if !settings.enabled {
            return Ok(registry);
        }
        for detector in detectors::tier1(settings)? {
            registry.detectors.push(detector);
        }
        Ok(registry)
    }

    /// Baut eine Registry ohne Detektoren, mit den Budgets aus den Einstellungen.
    #[must_use]
    pub fn empty(settings: &FindingsSettings) -> Self {
        Self {
            detectors: Vec::new(),
            protected: Vec::new(),
            ignored: settings.ignored_hashes.iter().copied().collect(),
            cap_bytes: settings.cap_bytes,
            max_decompress_ratio: settings.max_decompress_ratio,
        }
    }

    /// Legt einen weiteren Detektor dazu.
    pub fn register(&mut self, detector: Box<dyn Detector>) {
        self.detectors.push(detector);
    }

    /// Legt einen Detektor dazu, dessen Funde kein anderer verdrängt.
    ///
    /// Die Funde eines solchen Detektors laufen am Aufräumen vorbei: Sie
    /// fallen weder einem anderen Fund noch `findings.ignored_hashes` zum
    /// Opfer, und von gleichen Stellen bleibt eine. Umgekehrt verdrängen sie
    /// jeden Fund, der sie überlappt, außer einem durch Prüfsumme bestätigten
    /// (HUM-140, Review).
    pub fn register_protected(&mut self, detector: Box<dyn Detector>) {
        self.protected.push(detector);
    }

    /// Die Kennungen der eingehängten Detektoren.
    #[must_use]
    pub fn detector_ids(&self) -> Vec<&'static str> {
        self.detectors
            .iter()
            .chain(self.protected.iter())
            .map(|detector| detector.id())
            .collect()
    }

    /// Zerlegt die Anfrage, sammelt die Funde aller Detektoren und räumt auf.
    ///
    /// Aufgeräumt heißt: Bereiche der dekodierten Query zeigen wieder auf den
    /// Rohtext, Funde aus `findings.ignored_hashes` fallen weg, gleiche Stelle
    /// plus gleicher Bereich behält den höchsten Tier, und die Liste ist nach
    /// Ort und Bereichsanfang sortiert. Ausgenommen ist ein durch Prüfsumme
    /// bestätigter Fund: Kein überlappender Fund entfernt ihn, er fällt nur
    /// als Doppel seiner selbst weg.
    #[must_use]
    pub fn scan(&self, request: &HttpRequest, body: &[u8]) -> ScanReport {
        let targets =
            ScanTargets::from_request(request, body, self.cap_bytes, self.max_decompress_ratio);
        let mut findings = Vec::new();
        let mut protected = Vec::new();
        for target in targets.iter() {
            for detector in &self.detectors {
                for mut finding in detector.scan(&target.input) {
                    let Some(span) = target.map.map(finding.span.clone()) else {
                        continue;
                    };
                    finding.span = span;
                    if self.ignored.contains(&finding.value_hash) {
                        continue;
                    }
                    findings.push(finding);
                }
            }
            for detector in &self.protected {
                for mut finding in detector.scan(&target.input) {
                    let Some(span) = target.map.map(finding.span.clone()) else {
                        continue;
                    };
                    finding.span = span;
                    protected.push(finding);
                }
            }
        }
        ScanReport {
            findings: with_protected(tidy(findings), protected),
            truncated: targets.truncated(),
            diagnostics: targets.diagnostics().to_vec(),
        }
    }

    /// Wie [`DetectorRegistry::scan`], aber nur die Funde.
    #[must_use]
    pub fn scan_request(&self, request: &HttpRequest, body: &[u8]) -> Vec<Finding> {
        self.scan(request, body).findings
    }

    /// Sucht in einem Text des Menschen statt in einer Anfrage.
    ///
    /// Für die Notiz, die jemand beim Blocken an den Agenten richtet: Sie
    /// geht im Klartext in die 403-Antwort, und ein Schlüssel darin verlässt
    /// damit den Rechner (HUM-117). Es gilt dasselbe wie für jeden anderen
    /// Fund: Der Wert bleibt hier, nach außen gehen Art, Bereich und die
    /// ersten Zeichen.
    ///
    /// Kein Zerlegen, kein Entpacken, keine Grenze: Ein Text ist ein Text,
    /// und eine Notiz ist durch `sanitize_note` ohnehin auf 500 Zeichen
    /// begrenzt. Der Ort steht auf [`FindingLocation::Body`], weil ein Fund
    /// ohne Ort nicht aufzuräumen wäre; er wird nie zu einem `Finding` am
    /// Fluss, denn die gehören zur Anfrage. Der Aufrufer macht aus jedem Fund
    /// einen Befund.
    #[must_use]
    pub fn scan_note(&self, note: &str) -> Vec<Finding> {
        let input = ScanInput {
            location: FindingLocation::Body,
            bytes: note.as_bytes(),
            content_type: None,
        };
        let mut findings = Vec::new();
        for detector in &self.detectors {
            for finding in detector.scan(&input) {
                if self.ignored.contains(&finding.value_hash) {
                    continue;
                }
                findings.push(finding);
            }
        }
        // Auch eine Notiz kann den Zugangsschlüssel des Modells tragen, und
        // sie geht im Klartext an den Agenten und damit womöglich weiter
        // (HUM-140, Review): Die geschützten Detektoren suchen mit.
        let protected = self
            .protected
            .iter()
            .flat_map(|detector| detector.scan(&input))
            .collect();
        with_protected(tidy(findings), protected)
    }
}

/// Wie stark ein Fund wiegt, wenn zwei an derselben Stelle stehen.
///
/// Ein eigener Begriff des Nutzers schlägt alles: er hat ihn hinterlegt, weil
/// genau dieser Wert nie nach außen soll. Danach kommt die Prüfsumme, die den
/// Fund bestätigt, und zuletzt das bloße Muster.
///
/// Die Rangfolge entscheidet nur, welcher Fund oben steht und welche Muster
/// wegfallen. Einen durch Prüfsumme bestätigten Fund entfernt sie nie: An ihm
/// hängt die harte Sperre `hold.hard_block_checksum_secrets`, und ein
/// überlappender Begriff des Nutzers steht deshalb neben ihm, nicht an seiner
/// Stelle.
const fn tier_priority(tier: Tier) -> u8 {
    match tier {
        Tier::Regex => 0,
        Tier::Checksum => 1,
        Tier::UserTerm => 2,
    }
}

/// Sortierschlüssel eines Orts: erst die Header nach Namen, dann Query, dann Body.
fn location_key(location: &FindingLocation) -> (u8, &str) {
    match location {
        FindingLocation::Header(name) => (0, name.as_str()),
        FindingLocation::Query => (1, ""),
        FindingLocation::Body => (2, ""),
    }
}

/// Fügt die geschützten Funde zu den aufgeräumten.
///
/// Ein geschützter Fund verdrängt jeden anderen, der ihn am selben Ort
/// überlappt, so wie es das Aufräumen mit einem stärkeren tut: Sonst stünde
/// `sk-proj-…` einmal als Muster und einmal als bekannter Schlüssel da.
/// Selbst verdrängt wird er nie (HUM-140, Review). Ausgenommen ist ein durch
/// Prüfsumme bestätigter Fund: Er bleibt neben dem geschützten stehen
/// ([`is_confirmed`]).
fn with_protected(findings: Vec<Finding>, protected: Vec<Finding>) -> Vec<Finding> {
    let protected = distinct(protected);
    let mut merged: Vec<Finding> = findings
        .into_iter()
        .filter(|found| {
            is_confirmed(found)
                || !protected.iter().any(|guard| {
                    guard.location == found.location
                        && guard.span.start < found.span.end
                        && found.span.start < guard.span.end
                })
        })
        .collect();
    merged.extend(protected);
    merged.sort_by(|left, right| {
        location_key(&left.location)
            .cmp(&location_key(&right.location))
            .then(left.span.start.cmp(&right.span.start))
            .then(left.span.end.cmp(&right.span.end))
    });
    merged
}

/// Geschützte Funde ohne Doppel: Die rohe und die dekodierte Query zeigen
/// nach dem Zurückrechnen oft auf dieselbe Stelle.
fn distinct(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|left, right| {
        location_key(&left.location)
            .cmp(&location_key(&right.location))
            .then(left.span.start.cmp(&right.span.start))
            .then(right.span.end.cmp(&left.span.end))
    });
    findings.dedup_by(|later, kept| {
        later.location == kept.location && later.span.start < kept.span.end
    });
    findings
}

/// Ob ein Fund durch Prüfsumme bestätigt ist.
///
/// Ein solcher Fund übersteht jedes Aufräumen, gleich was ihn überlappt: Die
/// harte Sperre `hold.hard_block_checksum_secrets` sucht genau ihn, und ein
/// anderer Fund an derselben Stelle darf ihn nicht ersetzen. Wegfallen kann er
/// nur als Doppel seiner selbst (gleicher Ort, Bereich, Art und Tier).
fn is_confirmed(found: &Finding) -> bool {
    found.tier == Tier::Checksum
}

/// Ob `later` beim Deduplizieren gleicher Bereiche hinter `kept` wegfällt.
///
/// Die Liste ist so sortiert, dass an gleichem Bereich der höchste Tier vorn
/// steht. Ein bestätigter Fund fällt nur weg, wenn `kept` dasselbe meldet.
fn duplicates(later: &Finding, kept: &Finding) -> bool {
    if later.location != kept.location || later.span != kept.span {
        return false;
    }
    !is_confirmed(later) || (later.tier == kept.tier && later.kind == kept.kind)
}

/// Sortiert, dedupliziert und behält je Stelle den stärksten Fund.
///
/// Zwei Schritte: Bei gleichem Bereich bleibt der höchste Tier stehen, bei
/// überlappenden Bereichen ebenfalls. Der zweite Schritt ist der Grund, warum
/// eine Kreditkartennummer in einer Rufnummer nicht doppelt gemeldet wird: Ein
/// Fund, den der Mensch nicht auseinanderhalten kann, ist Lärm, und Lärm führt
/// dazu, dass er ohne Lesen freigibt. Ein durch Prüfsumme bestätigter Fund
/// fällt in keinem der beiden Schritte weg ([`is_confirmed`]).
fn tidy(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|left, right| {
        location_key(&left.location)
            .cmp(&location_key(&right.location))
            .then(left.span.start.cmp(&right.span.start))
            .then(left.span.end.cmp(&right.span.end))
            .then(tier_priority(right.tier).cmp(&tier_priority(left.tier)))
            .then(left.kind.cmp(&right.kind))
    });
    findings.dedup_by(|later, kept| duplicates(later, kept));
    suppress_overlapped(&mut findings);
    findings
}

/// Entfernt jeden Fund, den ein stärkerer am selben Ort überdeckt.
///
/// Durch Prüfsumme bestätigte Funde und Begriffe des Nutzers bleiben immer
/// stehen; wegfallen kann nur ein bloßes Muster.
///
/// Die Liste ist nach Ort und Bereichsanfang sortiert, deshalb reicht ein
/// Durchlauf je Ort: Aus den Funden höheren Tiers entsteht eine Liste
/// zusammengefasster, disjunkter Bereiche, und für jeden schwächeren Fund
/// entscheidet eine binäre Suche darin. Das bleibt `O(n log n)`, auch wenn ein
/// feindseliger Body Hunderttausende Funde erzeugt.
fn suppress_overlapped(findings: &mut Vec<Finding>) {
    let mut keep = vec![true; findings.len()];
    let mut start = 0usize;
    while start < findings.len() {
        let key = location_key(&findings[start].location);
        let mut end = start;
        while end < findings.len() && location_key(&findings[end].location) == key {
            end += 1;
        }
        let group = &findings[start..end];
        let stronger = merged_spans(group, 1);
        for (offset, found) in group.iter().enumerate() {
            if tier_priority(found.tier) > 0 {
                continue;
            }
            if overlaps_any(&stronger, &found.span) {
                keep[start + offset] = false;
            }
        }
        start = end;
    }

    let mut index = 0usize;
    findings.retain(|_| {
        let keeps = keep[index];
        index += 1;
        keeps
    });
}

/// Fasst die Bereiche der Funde ab diesem Tier zu disjunkten Bereichen zusammen.
///
/// Die Funde kommen nach Bereichsanfang sortiert herein, deshalb genügt ein
/// Durchlauf.
fn merged_spans(findings: &[Finding], min_priority: u8) -> Vec<Range<usize>> {
    let mut merged: Vec<Range<usize>> = Vec::new();
    for found in findings
        .iter()
        .filter(|found| tier_priority(found.tier) >= min_priority)
    {
        match merged.last_mut() {
            Some(last) if found.span.start < last.end => last.end = last.end.max(found.span.end),
            _ => merged.push(found.span.clone()),
        }
    }
    merged
}

/// Wahr, wenn der Bereich einen der disjunkten, sortierten Bereiche schneidet.
fn overlaps_any(merged: &[Range<usize>], span: &Range<usize>) -> bool {
    let after = merged.partition_point(|candidate| candidate.start < span.end);
    after > 0 && merged[after - 1].end > span.start
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use humanitl_core::http::HeaderName;
    use humanitl_core::{Finding, FindingKind, FindingLocation, Tier};

    use super::{location_key, tidy, with_protected};

    fn finding(location: FindingLocation, start: usize, tier: Tier, kind: FindingKind) -> Finding {
        Finding::new(kind, start..start + 4, location, tier, "wert")
    }

    #[test]
    fn the_highest_tier_survives_the_same_span() {
        let findings = tidy(vec![
            finding(FindingLocation::Body, 0, Tier::Regex, FindingKind::Email),
            finding(
                FindingLocation::Body,
                0,
                Tier::UserTerm,
                FindingKind::UserTerm("wert".to_owned()),
            ),
        ]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].tier, Tier::UserTerm);
    }

    #[test]
    fn a_checksum_beats_a_bare_pattern() {
        let findings = tidy(vec![
            finding(FindingLocation::Body, 0, Tier::Regex, FindingKind::Phone),
            finding(FindingLocation::Body, 0, Tier::Checksum, FindingKind::Iban),
        ]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::Iban);
    }

    #[test]
    fn a_weaker_finding_inside_a_stronger_one_falls_away() {
        let findings = tidy(vec![
            finding(FindingLocation::Body, 0, Tier::Checksum, FindingKind::Iban),
            Finding::new(
                FindingKind::Phone,
                1..3,
                FindingLocation::Body,
                Tier::Regex,
                "we",
            ),
        ]);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].kind, FindingKind::Iban);
    }

    /// Ein Begriff des Nutzers über einer IBAN, auf genau demselben Bereich
    /// und teilweise überlappend: Die IBAN bleibt jedes Mal stehen, neben dem
    /// Begriff. An ihr hängt `hold.hard_block_checksum_secrets`.
    #[test]
    fn a_user_term_over_a_checksum_finding_keeps_both() {
        let term = || FindingKind::UserTerm("wert".to_owned());
        let same_span = tidy(vec![
            finding(FindingLocation::Body, 0, Tier::Checksum, FindingKind::Iban),
            finding(FindingLocation::Body, 0, Tier::UserTerm, term()),
        ]);
        let kinds: Vec<FindingKind> = same_span.iter().map(|f| f.kind.clone()).collect();
        assert_eq!(kinds, vec![term(), FindingKind::Iban], "{same_span:?}");

        let overlapping = tidy(vec![
            finding(FindingLocation::Body, 0, Tier::Checksum, FindingKind::Iban),
            finding(FindingLocation::Body, 2, Tier::UserTerm, term()),
        ]);
        assert_eq!(overlapping.len(), 2, "{overlapping:?}");
        assert!(
            overlapping
                .iter()
                .any(|f| f.kind == FindingKind::Iban && f.tier == Tier::Checksum),
            "{overlapping:?}"
        );
    }

    /// Dieselbe IBAN zweimal an derselben Stelle (rohe und dekodierte Query)
    /// bleibt ein Fund; ein Muster über ihr fällt weiter weg.
    #[test]
    fn a_checksum_finding_falls_away_only_as_its_own_duplicate() {
        let findings = tidy(vec![
            finding(FindingLocation::Query, 0, Tier::Checksum, FindingKind::Iban),
            finding(FindingLocation::Query, 0, Tier::Checksum, FindingKind::Iban),
            finding(FindingLocation::Query, 1, Tier::Regex, FindingKind::Phone),
        ]);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].kind, FindingKind::Iban);
    }

    /// Ein geschützter Fund verdrängt ein Muster an seiner Stelle, einen
    /// durch Prüfsumme bestätigten Fund nicht.
    #[test]
    fn a_protected_finding_leaves_a_checksum_finding_in_place() {
        let merged = with_protected(
            vec![
                finding(FindingLocation::Body, 0, Tier::Checksum, FindingKind::Iban),
                finding(FindingLocation::Body, 8, Tier::Regex, FindingKind::Phone),
            ],
            vec![Finding::new(
                FindingKind::ApiKey("llm".to_owned()),
                2..10,
                FindingLocation::Body,
                Tier::Checksum,
                "geheimni",
            )],
        );
        let kinds: Vec<FindingKind> = merged.iter().map(|f| f.kind.clone()).collect();
        assert_eq!(
            kinds,
            vec![FindingKind::Iban, FindingKind::ApiKey("llm".to_owned())],
            "{merged:?}"
        );
    }

    #[test]
    fn an_overlap_at_another_location_keeps_both() {
        let findings = tidy(vec![
            finding(FindingLocation::Body, 0, Tier::Checksum, FindingKind::Iban),
            finding(FindingLocation::Query, 0, Tier::Regex, FindingKind::Phone),
        ]);
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn two_findings_of_the_same_tier_stay_side_by_side() {
        let findings = tidy(vec![
            finding(FindingLocation::Body, 0, Tier::Regex, FindingKind::Email),
            finding(FindingLocation::Body, 2, Tier::Regex, FindingKind::Email),
        ]);
        assert_eq!(findings.len(), 2);
    }

    #[test]
    fn findings_are_sorted_by_location_then_span() {
        let findings = tidy(vec![
            finding(FindingLocation::Body, 8, Tier::Regex, FindingKind::Email),
            finding(FindingLocation::Body, 0, Tier::Regex, FindingKind::Email),
            finding(FindingLocation::Query, 4, Tier::Regex, FindingKind::Email),
            finding(
                FindingLocation::Header(HeaderName::from_static("authorization")),
                0,
                Tier::Regex,
                FindingKind::Jwt,
            ),
        ]);
        let order: Vec<String> = findings
            .iter()
            .map(|found| format!("{}@{}", found.location, found.span.start))
            .collect();
        assert_eq!(
            order,
            vec![
                "header:authorization@0".to_owned(),
                "query@4".to_owned(),
                "body@0".to_owned(),
                "body@8".to_owned(),
            ]
        );
    }

    #[test]
    fn header_locations_sort_by_name() {
        assert!(
            location_key(&FindingLocation::Header(HeaderName::from_static("a")))
                < location_key(&FindingLocation::Header(HeaderName::from_static("b")))
        );
        assert!(location_key(&FindingLocation::Query) < location_key(&FindingLocation::Body));
    }
}
