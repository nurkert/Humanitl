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
    ignored: HashSet<[u8; 32]>,
    cap_bytes: usize,
    max_decompress_ratio: u32,
}

impl core::fmt::Debug for DetectorRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("DetectorRegistry")
            .field("detectors", &self.detector_ids())
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
            ignored: settings.ignored_hashes.iter().copied().collect(),
            cap_bytes: settings.cap_bytes,
            max_decompress_ratio: settings.max_decompress_ratio,
        }
    }

    /// Legt einen weiteren Detektor dazu.
    pub fn register(&mut self, detector: Box<dyn Detector>) {
        self.detectors.push(detector);
    }

    /// Die Kennungen der eingehängten Detektoren.
    #[must_use]
    pub fn detector_ids(&self) -> Vec<&'static str> {
        self.detectors
            .iter()
            .map(|detector| detector.id())
            .collect()
    }

    /// Zerlegt die Anfrage, sammelt die Funde aller Detektoren und räumt auf.
    ///
    /// Aufgeräumt heißt: Bereiche der dekodierten Query zeigen wieder auf den
    /// Rohtext, Funde aus `findings.ignored_hashes` fallen weg, gleiche Stelle
    /// plus gleicher Bereich behält den höchsten Tier, und die Liste ist nach
    /// Ort und Bereichsanfang sortiert.
    #[must_use]
    pub fn scan(&self, request: &HttpRequest, body: &[u8]) -> ScanReport {
        let targets =
            ScanTargets::from_request(request, body, self.cap_bytes, self.max_decompress_ratio);
        let mut findings = Vec::new();
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
        }
        ScanReport {
            findings: tidy(findings),
            truncated: targets.truncated(),
            diagnostics: targets.diagnostics().to_vec(),
        }
    }

    /// Wie [`DetectorRegistry::scan`], aber nur die Funde.
    #[must_use]
    pub fn scan_request(&self, request: &HttpRequest, body: &[u8]) -> Vec<Finding> {
        self.scan(request, body).findings
    }
}

/// Wie stark ein Fund wiegt, wenn zwei an derselben Stelle stehen.
///
/// Ein eigener Begriff des Nutzers schlägt alles: er hat ihn hinterlegt, weil
/// genau dieser Wert nie nach außen soll. Danach kommt die Prüfsumme, die den
/// Fund bestätigt, und zuletzt das bloße Muster.
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

/// Sortiert, dedupliziert und behält je Stelle den stärksten Fund.
///
/// Zwei Schritte: Bei gleichem Bereich bleibt der höchste Tier stehen, bei
/// überlappenden Bereichen ebenfalls. Der zweite Schritt ist der Grund, warum
/// eine Kreditkartennummer in einer Rufnummer nicht doppelt gemeldet wird: Ein
/// Fund, den der Mensch nicht auseinanderhalten kann, ist Lärm, und Lärm führt
/// dazu, dass er ohne Lesen freigibt.
fn tidy(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|left, right| {
        location_key(&left.location)
            .cmp(&location_key(&right.location))
            .then(left.span.start.cmp(&right.span.start))
            .then(left.span.end.cmp(&right.span.end))
            .then(tier_priority(right.tier).cmp(&tier_priority(left.tier)))
            .then(left.kind.cmp(&right.kind))
    });
    findings.dedup_by(|later, kept| later.location == kept.location && later.span == kept.span);
    suppress_overlapped(&mut findings);
    findings
}

/// Entfernt jeden Fund, den ein stärkerer am selben Ort überdeckt.
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
        let strongest = merged_spans(group, 2);
        let stronger = merged_spans(group, 1);
        for (offset, found) in group.iter().enumerate() {
            let dominant = match tier_priority(found.tier) {
                2 => continue,
                1 => &strongest,
                _ => &stronger,
            };
            if overlaps_any(dominant, &found.span) {
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

    use super::{location_key, tidy};

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
