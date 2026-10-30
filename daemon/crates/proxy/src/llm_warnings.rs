//! Welche Funde `LLM_005` in einem Lauf schon gemeldet hat (HUM-354).
//!
//! Agenten wie `OpenCode` und pi schicken mit jedem Zug das ganze Gespräch
//! noch einmal an das Modell. Steht ein Wert, der wie ein Geheimnis aussieht,
//! einmal im Kontext, trägt danach jede durchgereichte Anfrage denselben Fund,
//! und vor HUM-354 warnte `LLM_005` vor jeder einzelnen. Die Karte kam im Takt
//! der Anfragen wieder und sagte nichts Neues.
//!
//! Dieses Gedächtnis entstört **nur die Warnung**. Jeder Fund steht weiter an
//! seinem Fluss, in `Analyzed`, in der Aufzeichnung und in den Flow-Details,
//! genau wie vorher; nichts davon läuft hier durch. Gewarnt wird, sobald eine
//! Anfrage wenigstens einen Fund trägt, vor dem am selben Endpunkt noch nicht
//! oder vor mehr als [`WARNING_TTL`] zuletzt gewarnt wurde.
//!
//! # Was gemerkt wird
//!
//! Ein Fingerabdruck je Fund: die Art des Detektors und der SHA-256 über den
//! gefundenen Wert, den [`Finding::value_hash`] schon trägt, dazu der
//! Zeitpunkt der letzten Warnung. Den Klartext hat dieses Modul nie, und es
//! braucht keinen eigenen Hash: Derselbe Wert steht als derselbe Hash ohnehin
//! am Fluss und in der Aufzeichnung (HUM-256, HUM-271). Das Gedächtnis liegt
//! nur im Speicher des Daemons, wird nie geschrieben und nie über die Leitung
//! geschickt. Die gekürzte Anzeige ([`Finding::display_prefix`]) wird bewusst
//! nicht übernommen.
//!
//! # Wann es vergisst
//!
//! Eine Warnung, die niemand sieht, darf nicht für immer verstummen. Der
//! Ereignisstrom spielt nichts nach: War die Oberfläche beim ersten Auftreten
//! geschlossen oder gerade beim Wiederverbinden, erreichte die eine Warnung
//! niemanden. Deshalb vergisst das Gedächtnis auf drei Wegen, und jeder irrt
//! zur lauten Seite:
//!
//! - **Mit jedem Lauf.** Der Daemon hat eine [`SessionId`] für seine ganze
//!   Lebenszeit, aber jeder Start einer Sandbox ist ein neues Gespräch mit
//!   dem Modell. [`SessionSettings::set`](crate::session::SessionSettings::set)
//!   läuft genau dann und leert das Gedächtnis ([`LlmWarningMemory::reset`]).
//! - **Nach [`WARNING_TTL`].** Gemessen ab der letzten Warnung, nicht ab dem
//!   letzten Auftreten: Ein Fund, den der Agent mit jedem Zug weiterschickt,
//!   warnt alle fünfzehn Minuten einmal.
//! - **Am Deckel.** Höchstens [`MAX_FINGERPRINTS_PER_SCOPE`] Fingerabdrücke je
//!   Paar aus Sitzung und Endpunkt (`host:port`), der älteste fällt zuerst
//!   heraus, und höchstens [`MAX_SCOPES`] Paare, wobei das am längsten
//!   unbenutzte zuerst geht. So füllt der Agent den Speicher nicht.
//!
//! Ein abgelaufener oder herausgefallener Fund gilt beim nächsten Auftreten
//! wieder als neu und warnt noch einmal.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use humanitl_core::{Finding, FindingKind, SessionId};

/// So viele Fingerabdrücke merkt sich ein Paar aus Sitzung und Endpunkt.
///
/// Ein Gespräch mit einem Agenten bringt selten mehr als einige Dutzend
/// verschiedene Funde zusammen; viertausend reichen für jeden Lauf, den ein
/// Mensch führt, und kosten mit Art, Hash und Zeitpunkt weniger als ein
/// halbes Megabyte.
pub const MAX_FINGERPRINTS_PER_SCOPE: usize = 4096;

/// So viele Paare aus Sitzung und Endpunkt hält das Gedächtnis zugleich.
///
/// Ein Lauf hat einen Endpunkt; die übrigen Plätze nehmen weitere Endpunkte
/// und Sitzungen auf, bis eine neue sie verdrängt.
pub const MAX_SCOPES: usize = 16;

/// Nach dieser Zeit seit der letzten Warnung warnt derselbe Fund wieder.
///
/// Fünfzehn Minuten: lang genug, dass ein Agent, der alle paar Sekunden das
/// ganze Gespräch schickt, die Karte nicht im Takt seiner Züge erneuert, und
/// kurz genug, dass eine Warnung, die bei geschlossener Oberfläche verloren
/// ging, innerhalb einer Arbeitssitzung wiederkommt.
pub const WARNING_TTL: Duration = Duration::from_secs(15 * 60);

/// Ein Fund ohne seinen Wert: Art und Hash.
#[derive(Clone, PartialEq, Eq, Hash)]
struct Fingerprint {
    kind: FindingKind,
    value_hash: [u8; 32],
}

impl Fingerprint {
    fn of(finding: &Finding) -> Self {
        Self {
            kind: finding.kind.clone(),
            value_hash: finding.value_hash,
        }
    }
}

/// Was eine Anfrage an Funden trägt, gemessen an dem, was schon gemeldet ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Novelty {
    /// Verschiedene Funde, vor denen in diesem Paar noch nicht oder vor mehr
    /// als [`WARNING_TTL`] gewarnt wurde.
    pub new: usize,
    /// Alle Funde dieser Anfrage, wie der Fluss sie trägt.
    pub total: usize,
}

/// Die gemerkten Fingerabdrücke eines Paars.
#[derive(Default)]
struct Seen {
    /// Fingerabdruck und Zeitpunkt seiner letzten Warnung.
    warned: HashMap<Fingerprint, Instant>,
    /// Dieselben Fingerabdrücke in der Reihenfolge ihrer Warnungen, damit der
    /// älteste zuerst herausfällt. Ein Eintrag, dessen Zeitpunkt nicht mehr
    /// der in `warned` ist, ist durch eine neuere Warnung überholt und zählt
    /// nicht.
    order: VecDeque<(Fingerprint, Instant)>,
    /// Wann das Paar zuletzt gefragt wurde, als laufende Nummer.
    last_used: u64,
}

impl Seen {
    /// Merkt sich eine Warnung und hält den Deckel.
    fn remember(&mut self, print: Fingerprint, now: Instant) {
        self.warned.insert(print.clone(), now);
        self.order.push_back((print, now));
        while self.warned.len() > MAX_FINGERPRINTS_PER_SCOPE {
            let Some((dropped, at)) = self.order.pop_front() else {
                break;
            };
            if self.warned.get(&dropped) == Some(&at) {
                self.warned.remove(&dropped);
            }
        }
        // Überholte Einträge sammeln sich bei Funden, die nach Ablauf wieder
        // warnen; die Reihe bleibt so nie mehr als doppelt so lang wie der
        // Deckel.
        if self.order.len() > 2 * MAX_FINGERPRINTS_PER_SCOPE {
            let warned = &self.warned;
            self.order
                .retain(|(print, at)| warned.get(print) == Some(at));
        }
    }
}

#[derive(Default)]
struct Scopes {
    tick: u64,
    map: HashMap<(SessionId, String), Seen>,
}

/// Das Gedächtnis der Warnung `LLM_005`.
#[derive(Default)]
pub struct LlmWarningMemory {
    scopes: Mutex<Scopes>,
}

impl std::fmt::Debug for LlmWarningMemory {
    /// Nur die Zahl der Paare: Auch ein Hash gehört nicht in ein Log.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let scopes = self.scopes.lock().unwrap_or_else(PoisonError::into_inner);
        f.debug_struct("LlmWarningMemory")
            .field("scopes", &scopes.map.len())
            .finish()
    }
}

impl LlmWarningMemory {
    /// Ein leeres Gedächtnis.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Merkt sich die Funde einer Anfrage an `endpoint` (`host:port`) und
    /// sagt, wie viele davon neu sind.
    ///
    /// Wie [`LlmWarningMemory::observe_at`] mit der Uhr von jetzt.
    pub fn observe(&self, session: SessionId, endpoint: &str, findings: &[Finding]) -> Novelty {
        self.observe_at(session, endpoint, findings, Instant::now())
    }

    /// Merkt sich die Funde einer Anfrage zum Zeitpunkt `now`.
    ///
    /// Neu ist ein Fund, dessen Fingerabdruck in diesem Paar aus `session`
    /// und `endpoint` nicht steht oder dessen letzte Warnung mindestens
    /// [`WARNING_TTL`] zurückliegt; für ihn gilt `now` als neue Warnung.
    /// Derselbe Wert zweimal in einer Anfrage zählt als ein neuer Fund. Ein
    /// vergiftetes Lock wird übernommen statt aufgegeben: Schlimmstenfalls
    /// warnt eine Anfrage einmal zu oft.
    pub fn observe_at(
        &self,
        session: SessionId,
        endpoint: &str,
        findings: &[Finding],
        now: Instant,
    ) -> Novelty {
        let mut scopes = self.scopes.lock().unwrap_or_else(PoisonError::into_inner);
        scopes.tick += 1;
        let tick = scopes.tick;
        let key = (session, endpoint.to_owned());
        if !scopes.map.contains_key(&key) && scopes.map.len() >= MAX_SCOPES {
            let oldest = scopes
                .map
                .iter()
                .min_by_key(|(_, seen)| seen.last_used)
                .map(|(key, _)| key.clone());
            if let Some(oldest) = oldest {
                scopes.map.remove(&oldest);
            }
        }
        let seen = scopes.map.entry(key).or_default();
        seen.last_used = tick;
        let mut new = 0;
        for finding in findings {
            let print = Fingerprint::of(finding);
            let fresh = seen
                .warned
                .get(&print)
                .is_none_or(|at| now.saturating_duration_since(*at) >= WARNING_TTL);
            if fresh {
                new += 1;
                seen.remember(print, now);
            }
        }
        Novelty {
            new,
            total: findings.len(),
        }
    }

    /// Vergisst alles: Ein neuer Lauf ist ein neues Gespräch mit dem Modell.
    pub fn reset(&self) {
        let mut scopes = self.scopes.lock().unwrap_or_else(PoisonError::into_inner);
        scopes.map.clear();
    }

    /// Wie viele Fingerabdrücke das Paar gerade hält; für Tests des Deckels.
    #[must_use]
    pub fn remembered(&self, session: SessionId, endpoint: &str) -> usize {
        let scopes = self.scopes.lock().unwrap_or_else(PoisonError::into_inner);
        scopes
            .map
            .get(&(session, endpoint.to_owned()))
            .map_or(0, |seen| seen.warned.len())
    }

    /// Wie viele Paare das Gedächtnis gerade hält; für Tests des Deckels.
    #[must_use]
    pub fn scopes(&self) -> usize {
        self.scopes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .map
            .len()
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use humanitl_core::{FindingLocation, Tier};

    use super::*;

    const TOKEN: &str = "ghp_A1b2C3d4E5f6G7h8I9j0K1l2M3n4O5p6Q7r8";
    const MODEL: &str = "api.example:443";

    fn finding(value: &str) -> Finding {
        Finding::new(
            FindingKind::ApiKey("github".to_owned()),
            0..value.len(),
            FindingLocation::Body,
            Tier::Checksum,
            value,
        )
    }

    #[test]
    fn the_same_finding_is_new_only_once_per_session_and_endpoint() {
        let memory = LlmWarningMemory::new();
        let session = SessionId::new();
        let first = memory.observe(session, MODEL, &[finding(TOKEN)]);
        assert_eq!(first, Novelty { new: 1, total: 1 });
        let again = memory.observe(session, MODEL, &[finding(TOKEN), finding(TOKEN)]);
        assert_eq!(again, Novelty { new: 0, total: 2 });
        // Ein anderer Port desselben Hosts ist ein anderer Modellserver.
        // Rot, sobald der Schlüssel wieder nur den Host trägt.
        let other_port = memory.observe(session, "api.example:8443", &[finding(TOKEN)]);
        assert_eq!(other_port.new, 1);
        let other_host = memory.observe(session, "other.example:443", &[finding(TOKEN)]);
        assert_eq!(other_host.new, 1);
        let next_session = memory.observe(SessionId::new(), MODEL, &[finding(TOKEN)]);
        assert_eq!(next_session.new, 1);
    }

    #[test]
    fn a_finding_warns_again_after_the_ttl() {
        // Gemessen ab der letzten Warnung: Wer den Fund mit jedem Zug
        // weiterschickt, verlängert die Stille nicht.
        // Rot, sobald `observe_at` den Zeitpunkt nicht mehr vergleicht.
        let memory = LlmWarningMemory::new();
        let session = SessionId::new();
        let start = Instant::now();
        assert_eq!(
            memory
                .observe_at(session, MODEL, &[finding(TOKEN)], start)
                .new,
            1
        );
        let minute = Duration::from_secs(60);
        for step in 1..15 {
            let later = start + minute * step;
            assert_eq!(
                memory
                    .observe_at(session, MODEL, &[finding(TOKEN)], later)
                    .new,
                0,
                "still quiet after {step} minutes"
            );
        }
        let expired = start + WARNING_TTL;
        assert_eq!(
            memory
                .observe_at(session, MODEL, &[finding(TOKEN)], expired)
                .new,
            1,
            "warns again once the TTL has passed"
        );
        assert_eq!(
            memory
                .observe_at(session, MODEL, &[finding(TOKEN)], expired + minute)
                .new,
            0,
            "and the clock starts over with that warning"
        );
        assert_eq!(memory.remembered(session, MODEL), 1);
    }

    #[test]
    fn reset_forgets_everything() {
        let memory = LlmWarningMemory::new();
        let session = SessionId::new();
        memory.observe(session, MODEL, &[finding(TOKEN)]);
        memory.reset();
        assert_eq!(memory.scopes(), 0);
        assert_eq!(memory.observe(session, MODEL, &[finding(TOKEN)]).new, 1);
    }

    #[test]
    fn the_memory_holds_no_plaintext() {
        let memory = LlmWarningMemory::new();
        let session = SessionId::new();
        let found = finding(TOKEN);
        memory.observe(session, MODEL, std::slice::from_ref(&found));
        let scopes = memory.scopes.lock().unwrap();
        let seen = scopes.map.get(&(session, MODEL.to_owned())).unwrap();
        let mut stored = Vec::new();
        for print in seen.warned.keys() {
            stored.extend_from_slice(format!("{:?}", print.kind).as_bytes());
            stored.extend_from_slice(&print.value_hash);
        }
        let contains = |needle: &[u8]| stored.windows(needle.len()).any(|w| w == needle);
        assert!(!contains(TOKEN.as_bytes()), "the secret is not stored");
        assert!(
            !contains(found.display_prefix.trim_end_matches('…').as_bytes()),
            "not even its display prefix"
        );
        assert!(contains(&found.value_hash), "only its hash");
        drop(scopes);
        assert!(!format!("{memory:?}").contains("ghp_"));
    }

    #[test]
    fn the_oldest_fingerprint_falls_out_at_the_cap() {
        let memory = LlmWarningMemory::new();
        let session = SessionId::new();
        let values: Vec<String> = (0..=MAX_FINGERPRINTS_PER_SCOPE)
            .map(|i| format!("value-{i}"))
            .collect();
        for value in &values {
            memory.observe(session, MODEL, &[finding(value)]);
        }
        assert_eq!(
            memory.remembered(session, MODEL),
            MAX_FINGERPRINTS_PER_SCOPE
        );
        // Der erste ist herausgefallen und gilt wieder als neu, der letzte
        // steht noch.
        let last = memory.observe(session, MODEL, &[finding(&values[values.len() - 1])]);
        assert_eq!(last.new, 0);
        let first = memory.observe(session, MODEL, &[finding(&values[0])]);
        assert_eq!(first.new, 1, "a dropped fingerprint warns again");
        assert_eq!(
            memory.remembered(session, MODEL),
            MAX_FINGERPRINTS_PER_SCOPE
        );
    }

    #[test]
    fn the_order_stays_bounded_when_findings_warn_again() {
        // Jede Warnung nach Ablauf hinterlässt einen überholten Eintrag in
        // der Reihe; sie darf damit nicht ohne Ende wachsen.
        let memory = LlmWarningMemory::new();
        let session = SessionId::new();
        let mut now = Instant::now();
        for _ in 0..(3 * MAX_FINGERPRINTS_PER_SCOPE) {
            now += WARNING_TTL;
            memory.observe_at(session, MODEL, &[finding(TOKEN)], now);
        }
        let scopes = memory.scopes.lock().unwrap();
        let seen = scopes.map.get(&(session, MODEL.to_owned())).unwrap();
        assert_eq!(seen.warned.len(), 1);
        assert!(seen.order.len() <= 2 * MAX_FINGERPRINTS_PER_SCOPE + 1);
    }

    #[test]
    fn the_least_recently_used_scope_falls_out_at_the_cap() {
        let memory = LlmWarningMemory::new();
        let oldest = SessionId::new();
        memory.observe(oldest, MODEL, &[finding(TOKEN)]);
        for _ in 1..MAX_SCOPES {
            memory.observe(SessionId::new(), MODEL, &[finding(TOKEN)]);
        }
        assert_eq!(memory.scopes(), MAX_SCOPES);
        memory.observe(SessionId::new(), MODEL, &[finding(TOKEN)]);
        assert_eq!(memory.scopes(), MAX_SCOPES);
        assert_eq!(memory.remembered(oldest, MODEL), 0);
    }
}
