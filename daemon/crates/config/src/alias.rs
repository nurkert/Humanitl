//! Alte Schlüsselnamen, die weiter funktionieren.
//!
//! `backlog/CONVENTIONS.md` 4.4 hat alle Caps und Zeitgrenzen in die Gruppe
//! `limits` gezogen. Wer eine `config.toml` aus der Zeit davor hat, soll sie
//! nicht anfassen müssen. Ein Alias ist deshalb ein vollständiger Pfad, der auf
//! einen kanonischen Pfad zeigt; das Laden ersetzt ihn, bevor irgendetwas
//! gemischt wird.
//!
//! Der Alias liegt in einer anderen Gruppe als sein Ziel (`hold.body_cap_bytes`
//! gegen `limits.hold_body_cap_bytes`). `#[serde(alias)]` kann das nicht, weil
//! es nur Namen innerhalb einer Struktur umbenennt. Die Ersetzung passiert
//! darum in [`mod@crate::load`] auf der Ebene der Pfade.
//!
//! Regel bei Streit: innerhalb einer Ebene gewinnt der kanonische Schlüssel;
//! über Ebenen hinweg gilt die Präzedenz, auch wenn die höhere Ebene den alten
//! Namen benutzt. In beiden Fällen legt das Laden eine Warnung dazu, die den
//! Gewinner nennt. Still verlieren soll keiner der beiden.
//!
//! Daneben steht [`RETIRED`]: Pfade, die es einmal gab und die **ersatzlos**
//! entfallen sind. Sie haben kein Ziel, auf das ein Alias zeigen könnte, und
//! sie sind trotzdem kein Tippfehler — die Datei des Nutzers war gestern
//! gültig. Das Laden übergeht sie mit einer Warnung, statt den Start
//! abzubrechen (`backlog/CONVENTIONS.md` 4.25).

/// Ein alter Pfad und sein heutiger Name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Alias {
    /// Der alte Pfad, zum Beispiel `hold.body_cap_bytes`.
    pub old: &'static str,
    /// Der heutige Pfad, zum Beispiel `limits.hold_body_cap_bytes`.
    pub canonical: &'static str,
    /// Das Issue, das die Umbenennung entschieden hat.
    pub since: &'static str,
}

/// Alle Aliasse, sortiert nach dem alten Pfad.
pub static ALIASES: &[Alias] = &[
    Alias {
        old: "hold.body_cap_bytes",
        canonical: "limits.hold_body_cap_bytes",
        since: "HUM-057",
    },
    Alias {
        old: "ipc.event_buffer",
        canonical: "limits.event_buffer",
        since: "HUM-057",
    },
    Alias {
        old: "preview.cap_bytes",
        canonical: "limits.preview_cap_bytes",
        since: "HUM-057",
    },
    Alias {
        old: "preview.max_decompress_ratio",
        canonical: "limits.max_decompress_ratio",
        since: "HUM-057",
    },
    Alias {
        old: "recorder.max_body_bytes",
        canonical: "limits.recorder_max_body_bytes",
        since: "HUM-057",
    },
    Alias {
        old: "upstream.connect_timeout_secs",
        canonical: "limits.connect_timeout_secs",
        since: "HUM-057",
    },
];

/// Ein Pfad, den es einmal gab und der ersatzlos entfallen ist.
///
/// Kein Alias: Es gibt keinen Nachfolger, auf den das Laden ihn abbilden
/// könnte. Der Eintrag existiert, damit der Unterschied zwischen „gibt es
/// nicht" und „gab es einmal" sichtbar bleibt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retired {
    /// Der Pfad, wie er in alten Dateien steht.
    pub path: &'static str,
    /// Das Issue, das ihn entfernt hat.
    pub since: &'static str,
    /// Ein Satz, warum er weg ist; er steht im Befund.
    pub why: &'static str,
}

/// Alle entfallenen Pfade, sortiert.
///
/// Wer einen Schlüssel entfernt, hängt hier eine Zeile an. Ohne sie wird aus
/// dem entfernten Schlüssel beim nächsten Start ein harter `CONFIG_002`, und
/// der Nutzer trägt einen Fehler, den er nicht gemacht hat.
pub static RETIRED: &[Retired] = &[Retired {
    path: "limits.idle_timeout_secs",
    since: "HUM-101",
    why: "it described the same span as limits.header_timeout_secs, the one idle clock of the connection to the agent",
}];

/// Der Eintrag zu einem entfallenen Pfad.
#[must_use]
pub fn retired(path: &str) -> Option<&'static Retired> {
    RETIRED.iter().find(|entry| entry.path == path)
}

/// Der heutige Pfad zu einem alten, falls es einen gibt.
#[must_use]
pub fn canonical(path: &str) -> Option<&'static str> {
    ALIASES
        .iter()
        .find(|alias| alias.old == path)
        .map(|alias| alias.canonical)
}

/// Der Eintrag zu einem alten Pfad.
#[must_use]
pub fn lookup(path: &str) -> Option<&'static Alias> {
    ALIASES.iter().find(|alias| alias.old == path)
}

/// Alle alten Namen eines heutigen Pfades.
#[must_use]
pub fn old_names(canonical_path: &str) -> Vec<&'static str> {
    ALIASES
        .iter()
        .filter(|alias| alias.canonical == canonical_path)
        .map(|alias| alias.old)
        .collect()
}

/// Die Gruppen, die es nur noch als Alias gibt und deshalb nicht im Schema stehen.
///
/// `hold` und `recorder` tragen zwar auch alte Pfade, sind aber weiterhin
/// Gruppen des Schemas und stehen darum nicht in dieser Liste.
#[must_use]
pub fn legacy_groups() -> Vec<&'static str> {
    let mut groups: Vec<&'static str> = ALIASES
        .iter()
        .filter_map(|alias| alias.old.split_once('.').map(|(group, _)| group))
        .filter(|group| !crate::schema::field(group).is_some_and(|field| field.group))
        .collect();
    groups.sort_unstable();
    groups.dedup();
    groups
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::{ALIASES, RETIRED, canonical, legacy_groups, old_names, retired};

    #[test]
    fn aliases_are_sorted_and_unique() {
        let mut sorted: Vec<&str> = ALIASES.iter().map(|alias| alias.old).collect();
        let original = sorted.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted, original,
            "ALIASES must be sorted and free of doubles"
        );
    }

    #[test]
    fn no_alias_points_at_another_alias() {
        for alias in ALIASES {
            assert_eq!(
                canonical(alias.canonical),
                None,
                "{} points at another alias",
                alias.old
            );
        }
    }

    #[test]
    fn old_names_finds_both_directions() {
        assert_eq!(
            canonical("hold.body_cap_bytes"),
            Some("limits.hold_body_cap_bytes")
        );
        assert_eq!(
            old_names("limits.hold_body_cap_bytes"),
            vec!["hold.body_cap_bytes"]
        );
        assert_eq!(canonical("hold.timeout_secs"), None);
    }

    #[test]
    fn retired_paths_are_sorted_unique_and_gone_from_the_schema() {
        let mut sorted: Vec<&str> = RETIRED.iter().map(|entry| entry.path).collect();
        let original = sorted.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted, original,
            "RETIRED must be sorted and free of doubles"
        );
        for entry in RETIRED {
            assert!(
                !crate::schema::known_paths().contains(entry.path),
                "{} is listed as retired and is still in the schema",
                entry.path
            );
            assert_eq!(
                canonical(entry.path),
                None,
                "{} is retired and an alias at the same time; decide which",
                entry.path
            );
            assert!(!entry.why.is_empty() && !entry.since.is_empty());
        }
        assert!(retired("limits.idle_timeout_secs").is_some());
        assert!(retired("limits.header_timeout_secs").is_none());
        // Genau der Pfad, nicht sein Anfang und nicht seine Nachbarn: Ein
        // Vergleich mit `starts_with` machte aus jedem Tippfehler hinter einem
        // entfallenen Schlüssel eine Warnung, und die Milde für entfallene
        // Schlüssel griffe auf Tippfehler über (`backlog/CONVENTIONS.md` 4.25).
        assert!(retired("limits.idle_timeout_secsx").is_none());
        assert!(retired("limits.idle_timeout_secs.deeper").is_none());
        assert!(retired("limits.idle_timeout_sec").is_none());
        assert!(retired("").is_none());
    }

    #[test]
    fn legacy_groups_lists_the_groups_that_only_exist_as_alias() {
        // `hold` und `recorder` haben Aliasse, sind aber echte Gruppen.
        assert_eq!(legacy_groups(), vec!["ipc", "preview", "upstream"]);
    }
}
