//! Wie oft und seit wann ein Host in dieser Sitzung vorkam.
//!
//! Die Zähler leben im Prozess und sterben mit ihm. „Zum ersten Mal gesehen"
//! heißt deshalb immer „zum ersten Mal in dieser Sitzung", und die Oberfläche
//! muss es genauso schreiben: ein Host, den der Daemon gestern schon sah, wäre
//! sonst heute fälschlich neu. Was über Sitzungen hinweg gilt, weiß die
//! Aufzeichnung (`humanitl-recorder`), nicht dieser Speicher.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use humanitl_core::HostName;

/// Was der Speicher über einen Host weiß.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeenStats {
    /// Wann der Host in dieser Sitzung zum ersten Mal vorkam.
    pub first_seen: DateTime<Utc>,
    /// Wie oft er seither vorkam, die erste Beobachtung eingeschlossen.
    ///
    /// Der Zähler sättigt bei [`u32::MAX`], statt überzulaufen: eine Zahl, die
    /// wieder bei null anfängt, wäre eine falsche Angabe, und `DomainInfo`
    /// trägt ohnehin ein `uint32`.
    pub count: u32,
}

/// Zähler pro Host, für die laufende Sitzung.
#[derive(Debug, Default)]
pub struct SeenStore {
    map: DashMap<String, SeenStats>,
}

impl SeenStore {
    /// Ein leerer Speicher.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Verbucht eine Beobachtung und gibt den Stand danach zurück.
    ///
    /// Der Aufrufer ruft das genau einmal je eingetroffener Anfrage. Der
    /// Schlüssel ist die kanonische Form des Hosts (A-Label, klein, ohne Punkt
    /// am Ende, beziehungsweise die Adresse), damit zwei Schreibweisen
    /// desselben Namens nicht zwei Zeilen ergeben.
    ///
    /// Der Rückgabewert darf ignoriert werden: Wer nur zählen will, ruft die
    /// Funktion und liest den Stand später mit [`SeenStore::peek`].
    #[allow(clippy::must_use_candidate)]
    pub fn observe(&self, host: &HostName, at: DateTime<Utc>) -> SeenStats {
        let mut entry = self.map.entry(host.to_string()).or_insert(SeenStats {
            first_seen: at,
            count: 0,
        });
        entry.count = entry.count.saturating_add(1);
        // Kommt eine Beobachtung mit einem früheren Zeitpunkt herein, gilt der
        // frühere: „zum ersten Mal gesehen" soll nie nach hinten wandern.
        if at < entry.first_seen {
            entry.first_seen = at;
        }
        *entry
    }

    /// Der Stand eines Hosts, ohne ihn zu verbuchen.
    #[must_use]
    pub fn peek(&self, host: &HostName) -> Option<SeenStats> {
        self.map.get(&host.to_string()).map(|entry| *entry.value())
    }

    /// Wie viele verschiedene Hosts die Sitzung bisher gesehen hat.
    #[must_use]
    pub fn hosts(&self) -> usize {
        self.map.len()
    }

    /// Wahr, wenn noch nichts beobachtet wurde.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Die häufigsten Hosts, absteigend nach Zahl, bei Gleichstand nach Name.
    ///
    /// Für die Zusammenfassung, die das Domain-Panel ohne ausgewählten Flow
    /// zeigt. Die Reihenfolge ist bei gleichen Zahlen festgelegt, damit die
    /// Liste unter dem Zeiger nicht springt (`backlog/CONVENTIONS.md` 4.13).
    #[must_use]
    pub fn top(&self, limit: usize) -> Vec<(String, SeenStats)> {
        let mut all: Vec<(String, SeenStats)> = self
            .map
            .iter()
            .map(|entry| (entry.key().clone(), *entry.value()))
            .collect();
        all.sort_by(|(left_host, left), (right_host, right)| {
            right
                .count
                .cmp(&left.count)
                .then_with(|| left_host.cmp(right_host))
        });
        all.truncate(limit);
        all
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use chrono::{DateTime, TimeZone, Utc};
    use humanitl_core::HostName;

    use super::SeenStore;

    fn at(second: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 3, 12, 0, second).unwrap()
    }

    fn host(name: &str) -> HostName {
        HostName::parse(name).unwrap()
    }

    #[test]
    fn the_first_observation_counts_as_one() {
        let store = SeenStore::new();
        let stats = store.observe(&host("api.github.com"), at(1));
        assert_eq!(stats.count, 1);
        assert_eq!(stats.first_seen, at(1));
    }

    #[test]
    fn further_observations_keep_the_first_time() {
        let store = SeenStore::new();
        store.observe(&host("api.github.com"), at(1));
        let stats = store.observe(&host("api.github.com"), at(9));
        assert_eq!(stats.count, 2);
        assert_eq!(stats.first_seen, at(1));
        assert_eq!(store.hosts(), 1);
    }

    #[test]
    fn two_spellings_of_one_name_are_one_host() {
        let store = SeenStore::new();
        store.observe(&host("API.GitHub.com"), at(1));
        store.observe(&host("api.github.com."), at(2));
        assert_eq!(store.hosts(), 1);
        assert_eq!(store.peek(&host("api.github.com")).unwrap().count, 2);
    }

    #[test]
    fn top_sorts_by_count_then_by_name() {
        let store = SeenStore::new();
        for _ in 0..3 {
            store.observe(&host("registry.npmjs.org"), at(1));
        }
        store.observe(&host("crates.io"), at(2));
        store.observe(&host("pypi.org"), at(3));
        let top = store.top(2);
        assert_eq!(top[0].0, "registry.npmjs.org");
        assert_eq!(top[0].1.count, 3);
        assert_eq!(top[1].0, "crates.io");
    }

    #[test]
    fn peek_does_not_count() {
        let store = SeenStore::new();
        assert!(store.peek(&host("crates.io")).is_none());
        assert!(store.is_empty());
    }
}
